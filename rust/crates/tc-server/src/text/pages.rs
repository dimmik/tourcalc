//! The ten pages.
//!
//! Each one loads the tour, works its figures out with `tc-core`, and writes a table. The
//! shape of every page - what it shows and in what order - follows the C# Razor pages it
//! replaces, down to the wording, because the two are meant to be the same interface.

use super::forms::Fields;
use super::render::{esc, money, page, short};
use super::{Reader, COOKIE};
use crate::api::write as api_write;
use crate::auth::AuthData;
use crate::fields;
use crate::state::Shared;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use std::collections::HashMap;
use std::sync::Arc;
use tc_core::{
    calculate, settlement_summary, split_family, suggest_settlement, Cents, Kind, Options, Person,
    PersonId, Spending, SpendingId, Split, Tour, TourId, Transfer, MINIMUM_MEANINGFUL,
};

// --- getting a tour on screen -------------------------------------------------------------

/// Everything the four tour pages share: the tour, its headline figures, and the payments
/// that are still outstanding.
struct Loaded {
    tour: Arc<Tour>,
    id: String,
    currency: String,
    total_spent: Cents,
    expenses: usize,
    transfers: Vec<Transfer>,
    left_to_settle: Cents,
    /// Every suggested payment including the family ones and the dust, which is what
    /// `will_pay` has to see to decide whether somebody has anything to hand over.
    all_transfers: Vec<Transfer>,
}

impl Loaded {
    fn header(&self, active: &str) -> String {
        let t = &self.tour;
        let settle = if self.transfers.is_empty() {
            "everyone is settled up".to_owned()
        } else {
            format!(
                "left to settle <strong>{}</strong> {}",
                money(self.left_to_settle),
                esc(&self.currency)
            )
        };
        let flag = |name: &str, text: &str| {
            if fields::bool_of(t, name) {
                format!(" &middot; {text}")
            } else {
                String::new()
            }
        };
        let tab = |href: String, label: &str| {
            if label == active {
                format!("<a href=\"{href}\"><strong>{label}</strong></a>")
            } else {
                format!("<a href=\"{href}\">{label}</a>")
            }
        };
        format!(
            r#"<h1>{name}</h1>
<p><strong>{spent}</strong> {cur} spent &middot; {people} people &middot; {expenses} expenses &middot; {settle}{fin}{arch}</p>
<nav>{balance} {ppl} {exp} {stats}</nav>
<hr />"#,
            name = esc(&t.name),
            spent = money(self.total_spent),
            cur = esc(&self.currency),
            people = t.persons.len(),
            expenses = self.expenses,
            fin = flag(fields::FINALIZING, "settling up"),
            arch = flag(fields::ARCHIVED, "archived"),
            balance = tab(format!("/t/{}", self.id), "Balance"),
            ppl = tab(format!("/t/{}/people", self.id), "People"),
            exp = tab(format!("/t/{}/spend", self.id), "Expenses"),
            stats = tab(format!("/t/{}/stats", self.id), "Stats"),
        )
    }

    /// What the balances table shows: people who settle up for themselves, and what each
    /// of them hands over.
    fn balances(&self) -> Vec<(&Person, Cents)> {
        settlement_summary(&self.tour, &self.all_transfers)
            .into_iter()
            .filter_map(|(id, amount)| self.tour.person(&id).map(|p| (p, amount)))
            .collect()
    }
}

/// Loads a tour for a reader, or says what to send instead.
///
/// The error is boxed because it is a whole HTTP response - much bigger than the tour it
/// stands in for, and every successful call would otherwise carry that size about with it.
fn load(state: &Shared, auth: &AuthData, id: &str) -> Result<Loaded, Box<Response>> {
    let tour = state
        .store
        .get(&TourId::new(id.to_owned()))
        .filter(|t| auth.may_see(&fields::access_code(t)))
        .ok_or_else(|| {
            Box::new(not_found(
                "No tour with that id, or not one this code may see.",
            ))
        })?;

    let currency = if tour.currencies.len() > 1 {
        tour.currency().name.clone()
    } else {
        String::new()
    };

    let real: Vec<&Spending> = tour
        .spendings
        .iter()
        .filter(|s| s.kind.counts(false))
        .collect();
    let total_spent: Cents = real
        .iter()
        // A payback has no category and is not an expense.
        .filter(|s| !s.category.trim().is_empty())
        .map(|s| tour.amount_in_current(s))
        .sum();
    let expenses = real.len();

    let all_transfers = suggest_settlement(&tour).unwrap_or_default();
    let (_family, between) = split_family(&all_transfers);
    let mut transfers: Vec<Transfer> = between
        .into_iter()
        .filter(|t| tour.convert(t.amount, &t.currency).abs().0 > MINIMUM_MEANINGFUL)
        .cloned()
        .collect();
    transfers.sort_by_key(|t| -tour.convert(t.amount, &t.currency).0);
    let left_to_settle: Cents = transfers
        .iter()
        .map(|t| tour.convert(t.amount, &t.currency))
        .sum();

    Ok(Loaded {
        id: id.to_owned(),
        currency,
        total_spent,
        expenses,
        transfers,
        left_to_settle,
        all_transfers,
        tour,
    })
}

fn not_found(why: &str) -> Response {
    (
        axum::http::StatusCode::NOT_FOUND,
        page(
            "Not found",
            &format!("<h1>Not found</h1><p>{}</p>", esc(why)),
        ),
    )
        .into_response()
}

/// Sends a signed-out reader to the login form, remembering where they were going.
fn to_login(going_to: &str) -> Response {
    let next: String = form_urlencoded::byte_serialize(going_to.as_bytes()).collect();
    Redirect::to(&format!("/t/login?next={next}")).into_response()
}

// --- signing in ---------------------------------------------------------------------------

#[derive(serde::Deserialize, Default)]
pub struct Next {
    #[serde(default)]
    next: String,
    #[serde(default)]
    failed: String,
}

pub async fn login_form(Query(q): Query<Next>) -> Html<String> {
    let failed = if q.failed.is_empty() {
        String::new()
    } else {
        "<p><strong>That code was not accepted.</strong> Check it and try again.</p>".to_owned()
    };
    page(
        "Log in",
        &format!(
            r#"<h1>Log in</h1>
{failed}
<form method="post">
<input type="hidden" name="next" value="{next}" />
<p><label for="code">Access code</label><br />
<input type="text" id="code" name="Code" size="24" autocomplete="off" /></p>
<p><label for="scope">Scope</label>
<select id="scope" name="Scope">
<option value="code" selected>access code</option>
<option value="admin">master key</option>
</select></p>
<p><button type="submit">Log in</button></p>
</form>
<p class="muted">Opening a tour share link logs you in by itself &mdash; ask whoever shares
the tour to send it again.</p>"#,
            next = esc(&q.next),
        ),
    )
}

pub async fn login(State(state): State<Shared>, body: String) -> Response {
    let form = Fields::parse(&body);
    let code = form.text("Code");
    let scope = form.text("Scope");
    let next = form.text("next");

    let auth = match scope {
        "admin" if !state.master_key.is_empty() && code == state.master_key => AuthData::master(),
        "code" if !code.is_empty() => AuthData::for_code_md5(crate::auth::code_md5(code)),
        _ => {
            return Redirect::to(&format!(
                "/t/login?failed=1&next={}",
                form_urlencoded::byte_serialize(next.as_bytes()).collect::<String>()
            ))
            .into_response()
        }
    };

    let token = state.signer.issue(scope, &auth, state.token_valid_minutes);
    let go = if next.starts_with("/t") { next } else { "/t" };
    signed_in(token, state.token_valid_minutes, go)
}

/// `GET /t/goto/{code}/{tour}` - the text twin of the app's share link.
pub async fn goto(
    State(state): State<Shared>,
    Path(path): Path<HashMap<String, String>>,
) -> Response {
    let md5 = path.get("md5").cloned().unwrap_or_default();
    let auth = AuthData::for_code_md5(md5.to_uppercase());
    let token = state.signer.issue("code", &auth, state.token_valid_minutes);
    let go = match path.get("tour") {
        Some(t) if !t.is_empty() => format!("/t/{t}"),
        _ => "/t".to_owned(),
    };
    signed_in(token, state.token_valid_minutes, &go)
}

/// Sets the cookie and sends the reader on.
fn signed_in(token: String, valid_minutes: i64, go_to: &str) -> Response {
    let cookie = format!(
        // Secure is deliberately not set: the app is served over plain http in development,
        // and a cookie marked Secure would simply be dropped there.
        "{COOKIE}={token}; Path=/t; HttpOnly; SameSite=Strict; Max-Age={}",
        valid_minutes.max(0) * 60
    );
    (
        [(axum::http::header::SET_COOKIE, cookie)],
        Redirect::to(go_to),
    )
        .into_response()
}

pub async fn logout() -> Response {
    (
        [(
            axum::http::header::SET_COOKIE,
            format!("{COOKIE}=; Path=/t; HttpOnly; SameSite=Strict; Max-Age=0"),
        )],
        Redirect::to("/t/login"),
    )
        .into_response()
}

// --- the list -----------------------------------------------------------------------------

pub async fn index(State(state): State<Shared>, Reader(auth): Reader) -> Response {
    if auth.kind == "None" {
        return to_login("/t");
    }

    let tours = state
        .store
        .list(&|t: &Tour| auth.may_see(&fields::access_code(t)));

    let body = if tours.is_empty() {
        "<h1>Tours</h1><p>No tours are visible with this access code.</p>".to_owned()
    } else {
        let rows: String = tours
            .iter()
            .map(|t| {
                let state_text = [
                    fields::bool_of(t, fields::FINALIZING).then_some("settling up"),
                    fields::bool_of(t, fields::ARCHIVED).then_some("archived"),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ");
                format!(
                    "<tr><td><a href=\"/t/{id}\">{name}</a></td><td class=\"n\">{people}</td><td class=\"n\">{days}</td><td>{state_text}</td></tr>",
                    id = esc(t.id.as_str()),
                    name = esc(&t.name),
                    people = t.persons.len(),
                    days = days_of(t),
                )
            })
            .collect();
        format!(
            r#"<h1>Tours</h1>
<table><thead><tr><th>Tour</th><th class="n">People</th><th class="n">Days</th><th>State</th></tr></thead>
<tbody>{rows}</tbody></table>"#
        )
    };
    page("Tours", &body).into_response()
}

/// How many days the tour covers, from the first spending to the last.
fn days_of(tour: &Tour) -> i64 {
    let days: Vec<&str> = tour
        .spendings
        .iter()
        .filter(|s| s.kind.counts(false))
        .filter_map(|s| s.day())
        .collect();
    match (days.iter().min(), days.iter().max()) {
        (Some(first), Some(last)) => days_between(first, last),
        _ => 0,
    }
}

/// Whole days between two `YYYY-MM-DD` strings, inclusive of both ends.
///
/// Done by hand rather than with a date library: the strings are already sorted, and the
/// only arithmetic wanted is a difference in days. A calendar crate for this would be a
/// dependency carrying leap seconds and time zones to answer "how long was the trip".
pub fn days_between(first: &str, last: &str) -> i64 {
    let parse = |s: &str| -> Option<(i64, i64, i64)> {
        let mut parts = s.split('-');
        Some((
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.get(..2)?.parse().ok()?,
        ))
    };
    // Days since an arbitrary epoch, by the civil-from-days algorithm: exact for any date
    // in the proleptic Gregorian calendar, and eight lines.
    let to_days = |(y, m, d): (i64, i64, i64)| -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    };
    match (parse(first), parse(last)) {
        (Some(a), Some(b)) => (to_days(b) - to_days(a) + 1).max(1),
        _ => 0,
    }
}

// --- one tour -----------------------------------------------------------------------------

#[derive(serde::Deserialize, Default)]
pub struct Recorded {
    #[serde(default)]
    recorded: String,
}

pub async fn tour(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path(id): Path<String>,
    Query(q): Query<Recorded>,
) -> Response {
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };

    let recorded = if q.recorded.is_empty() {
        String::new()
    } else {
        format!("<p><strong>Recorded:</strong> {}</p>", esc(&q.recorded))
    };

    let payments = if loaded.transfers.is_empty() {
        "<p>Nothing left to settle.</p>".to_owned()
    } else {
        let rows: String = loaded
            .transfers
            .iter()
            .map(|t| {
                format!(
                    r#"<tr><td>{from} &rarr; {to}</td><td class="n">{amount}</td>
<td><form method="post" action="/t/{id}/markpaid">
<input type="hidden" name="transferId" value="{tid}" />
<button type="submit">Mark paid</button></form></td></tr>"#,
                    from = esc(&name_of(&loaded.tour, &t.from)),
                    to = esc(&name_of(&loaded.tour, &t.to)),
                    amount = money(loaded.tour.convert(t.amount, &t.currency)),
                    id = esc(&loaded.id),
                    tid = esc(&api_write::transfer_id(t)),
                )
            })
            .collect();
        format!(
            r#"<p class="muted">Suggested payments &mdash; none of them has happened yet.</p>
<table><thead><tr><th>Payment</th><th class="n">Amount</th><th></th></tr></thead>
<tbody>{rows}</tbody></table>"#
        )
    };

    let balances = loaded.balances();
    let balance_table = if balances.is_empty() {
        "<p>Everyone is square.</p>".to_owned()
    } else {
        let rows: String = balances
            .iter()
            .map(|(p, amount)| {
                format!(
                    "<tr><td>{name}</td><td>{word} {sum}</td></tr>",
                    name = esc(&p.name),
                    word = if amount.0 > 0 { "owes" } else { "gets" },
                    sum = money(amount.abs()),
                )
            })
            .collect();
        format!(
            r#"<table><thead><tr><th>Person</th><th>Balance</th></tr></thead>
<tbody>{rows}</tbody></table>"#
        )
    };

    page(
        &loaded.tour.name.clone(),
        &format!(
            "{header}{recorded}<h2>Who pays whom</h2>{payments}<h2>Balances</h2>{balance_table}",
            header = loaded.header("Balance"),
        ),
    )
    .into_response()
}

fn name_of(tour: &Tour, id: &PersonId) -> String {
    tour.person(id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "n/a".to_owned())
}

/// Records one of the suggested payments as having happened.
pub async fn mark_paid(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path(id): Path<String>,
    body: String,
) -> Response {
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };

    let wanted = Fields::parse(&body).text("transferId").to_owned();
    let Some(transfer) = loaded
        .all_transfers
        .iter()
        .find(|t| api_write::transfer_id(t) == wanted)
    else {
        // The tour changed under the reader and this payment is no longer suggested.
        return Redirect::to(&format!("/t/{id}")).into_response();
    };

    // A payback, not an expense: no category, so it does not count as spending.
    let payment = Spending {
        id: SpendingId::new(api_write::new_spending_id()),
        description: transfer.description.clone(),
        category: String::new(),
        amount: transfer.amount,
        currency: transfer.currency.clone(),
        from: transfer.from.clone(),
        split: Split::Equally(vec![transfer.to.clone()]),
        remembered_split: None,
        kind: Kind::Real,
        extras: Default::default(),
    };

    let note = format!(
        "{} → {}, {}",
        name_of(&loaded.tour, &transfer.from),
        name_of(&loaded.tour, &transfer.to),
        money(loaded.tour.convert(transfer.amount, &transfer.currency)),
    );

    let mut next = (*loaded.tour).clone();
    // The suggested payments are recomputed from the real ones and must never be stored.
    next.spendings.retain(|s| s.kind != Kind::Planned);
    next.spendings.push(payment);

    match save(&state, &loaded.tour, next) {
        Ok(()) => Redirect::to(&format!(
            "/t/{id}?recorded={}",
            form_urlencoded::byte_serialize(note.as_bytes()).collect::<String>()
        ))
        .into_response(),
        Err(response) => *response,
    }
}

/// Stores a changed tour, keeping a version of what it replaced.
///
/// Goes through the same [`crate::store::TourStore::replace`] the API uses, so a text post
/// and an app tab cannot overwrite each other unnoticed - the C#'s text pages save without
/// that check and say so in a comment.
fn save(state: &Shared, previous: &Tour, mut next: Tour) -> Result<(), Box<Response>> {
    fields::set(&mut next, fields::STATE, api_write::new_state_guid().into());
    let keep = state.versioning;
    let make_version = |was: &Tour| -> Option<Tour> {
        if !keep {
            return None;
        }
        let comment = crate::versions::describe_change(was, &next)?;
        Some(api_write::version_of(was, comment))
    };

    state
        .store
        .replace(
            &previous.id,
            &fields::str_of(previous, fields::STATE),
            next.clone(),
            &make_version,
        )
        .map_err(|_| {
            Box::new(
                (
                    axum::http::StatusCode::CONFLICT,
                    page(
                        "Somebody was quicker",
                        "<h1>Somebody was quicker</h1><p>This tour changed while you were \
                     looking at it, so nothing was saved. Go back, reload the page and \
                     make the change again.</p>",
                    ),
                )
                    .into_response(),
            )
        })
}

// --- people -------------------------------------------------------------------------------

pub async fn people(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path(id): Path<String>,
) -> Response {
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}/people"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };
    let tour = &loaded.tour;

    let body = if tour.persons.is_empty() {
        "<h2>People</h2><p>Nobody in this tour yet.</p>".to_owned()
    } else {
        let balances = calculate(tour, Options::default());
        // Everyone under whoever pays for them, so a family reads as a family.
        let mut order: Vec<&Person> = Vec::new();
        for p in tour.persons.iter().filter(|p| p.parent.is_none()) {
            order.push(p);
            order.extend(
                tour.persons
                    .iter()
                    .filter(|k| k.parent.as_ref() == Some(&p.id)),
            );
        }
        // Anybody whose payer is not in the tour at all would otherwise vanish from the
        // page while still counting in the arithmetic.
        order.extend(tour.persons.iter().filter(|p| {
            p.parent
                .as_ref()
                .is_some_and(|id| tour.person(id).is_none())
        }));

        let rows: String = order
            .iter()
            .map(|p| {
                let settle = tc_core::will_pay(tour, &loaded.all_transfers, &p.id, Cents::ZERO);
                let settle = if settle.abs().0 > MINIMUM_MEANINGFUL {
                    settle
                } else {
                    Cents::ZERO
                };
                let b = balances.get(&p.id);
                format!(
                    r#"<tr><td>{lead}{name}</td><td class="n">{weight}</td><td class="n">{paid}</td>
<td class="n">{charged}</td><td>{balance}</td><td><a href="/t/{tour}/people/edit/{pid}">edit</a></td></tr>"#,
                    lead = if p.parent.is_some() { "└ " } else { "" },
                    name = esc(&p.name),
                    weight = p.weight,
                    paid = money(b.map(|b| b.spent).unwrap_or_default()),
                    charged = money(b.map(|b| b.received).unwrap_or_default()),
                    balance = if settle.is_zero() {
                        "settled".to_owned()
                    } else {
                        format!(
                            "{} {}",
                            if settle.0 > 0 { "owes" } else { "gets" },
                            money(settle.abs())
                        )
                    },
                    tour = esc(&loaded.id),
                    pid = esc(p.id.as_str()),
                )
            })
            .collect();

        format!(
            r#"<h2>People</h2>
<p><a href="/t/{id}/people/edit">Add a person</a></p>
<table><thead><tr><th>Person</th><th class="n">Weight</th><th class="n">Paid</th>
<th class="n">Charged</th><th>Balance</th><th></th></tr></thead>
<tbody>{rows}</tbody></table>
<p class="muted">Total weight {weight}. A person listed under another is paid for by them
and settles through them.</p>"#,
            id = esc(&loaded.id),
            weight = tour.total_weight(),
        )
    };

    page(
        &format!("{} · people", loaded.tour.name),
        &format!("{}{}", loaded.header("People"), body),
    )
    .into_response()
}

pub async fn person_form(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path(path): Path<HashMap<String, String>>,
) -> Response {
    let id = path.get("id").cloned().unwrap_or_default();
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}/people/edit"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };
    let person = path
        .get("person")
        .and_then(|p| loaded.tour.person(&PersonId::new(p.clone())));

    person_page(&loaded, person, None).into_response()
}

fn person_page(loaded: &Loaded, person: Option<&Person>, error: Option<&str>) -> Html<String> {
    let is_new = person.is_none();
    let title = if is_new {
        "Add a person"
    } else {
        "Edit person"
    };
    let name = person.map(|p| p.name.as_str()).unwrap_or("");
    let weight = person.map(|p| p.weight).unwrap_or(100);
    let parent = person.and_then(|p| p.parent.clone());

    // Somebody cannot be paid for by themselves, nor by anybody they pay for.
    let options: String = loaded
        .tour
        .persons
        .iter()
        .filter(|p| person.is_none_or(|me| p.id != me.id))
        .filter(|p| person.is_none_or(|me| p.parent.as_ref() != Some(&me.id)))
        .map(|p| {
            format!(
                "<option value=\"{id}\"{sel}>{name}</option>",
                id = esc(p.id.as_str()),
                sel = if parent.as_ref() == Some(&p.id) {
                    " selected"
                } else {
                    ""
                },
                name = esc(&p.name),
            )
        })
        .collect();

    let action = match person {
        Some(p) => format!("/t/{}/people/edit/{}", loaded.id, p.id.as_str()),
        None => format!("/t/{}/people/edit", loaded.id),
    };
    let delete = match person {
        Some(p) => format!(
            r#"<form method="post" action="/t/{tour}/people/delete/{pid}">
<button type="submit">Delete this person</button>
<span class="muted">Their expenses go with them.</span></form>"#,
            tour = esc(&loaded.id),
            pid = esc(p.id.as_str()),
        ),
        None => String::new(),
    };
    let error = error
        .map(|e| format!("<p><strong>{}</strong></p>", esc(e)))
        .unwrap_or_default();

    page(
        title,
        &format!(
            r#"{header}<h2>{title}</h2>{error}
<form method="post" action="{action}">
<p><label for="name">Name</label><br />
<input type="text" id="name" name="Name" value="{name}" size="30" /></p>
<p><label for="weight">Share of the common expenses</label>
<input type="number" id="weight" name="Weight" value="{weight}" size="6" /><br />
<span class="muted">A full share is 100. Someone on 50 pays for half as much of everything
shared.</span></p>
<p><label for="parent">Paid for by</label>
<select id="parent" name="ParentId">
<option value="">pays for themselves</option>
{options}
</select></p>
<p><button type="submit">{button}</button>
<a href="/t/{tour}/people">Cancel</a></p>
</form>
{delete}"#,
            header = loaded.header("People"),
            name = esc(name),
            tour = esc(&loaded.id),
            button = if is_new { "Add person" } else { "Save" },
        ),
    )
}

pub async fn person_save(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path(path): Path<HashMap<String, String>>,
    body: String,
) -> Response {
    let id = path.get("id").cloned().unwrap_or_default();
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}/people"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };

    let form = Fields::parse(&body);
    let name = form.text("Name").to_owned();
    let weight = form.number("Weight").unwrap_or(100);
    let parent = form.text("ParentId").to_owned();

    let existing = path
        .get("person")
        .map(|p| PersonId::new(p.clone()))
        .filter(|p| loaded.tour.person(p).is_some());

    if name.trim().is_empty() {
        let person = existing.as_ref().and_then(|p| loaded.tour.person(p));
        return person_page(&loaded, person, Some("A name, please.")).into_response();
    }
    if weight <= 0 {
        let person = existing.as_ref().and_then(|p| loaded.tour.person(p));
        return person_page(
            &loaded,
            person,
            Some("A weight has to be more than nothing."),
        )
        .into_response();
    }

    let mut next = (*loaded.tour).clone();
    next.spendings.retain(|s| s.kind != Kind::Planned);
    let parent = (!parent.is_empty()).then(|| PersonId::new(parent));

    match existing {
        Some(pid) => {
            if let Some(p) = next.persons.iter_mut().find(|p| p.id == pid) {
                p.name = name.trim().to_owned();
                p.weight = weight as i32;
                p.parent = parent;
            }
        }
        None => next.persons.push(Person {
            id: PersonId::new(api_write::new_spending_id()),
            name: name.trim().to_owned(),
            weight: weight as i32,
            parent,
            group: None,
            extras: Default::default(),
        }),
    }

    match save(&state, &loaded.tour, next) {
        Ok(()) => Redirect::to(&format!("/t/{id}/people")).into_response(),
        Err(response) => *response,
    }
}

pub async fn person_delete(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path((id, person)): Path<(String, String)>,
) -> Response {
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}/people"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };

    // The same rules the app applies: what they paid for goes, and nobody is left pointing
    // at somebody who is no longer there.
    let next = crate::text::pages::without_person(&loaded.tour, &PersonId::new(person));
    match save(&state, &loaded.tour, next) {
        Ok(()) => Redirect::to(&format!("/t/{id}/people")).into_response(),
        Err(response) => *response,
    }
}

/// A tour with one person, and everything that would now point at nobody, removed.
pub fn without_person(tour: &Tour, id: &PersonId) -> Tour {
    let mut next = tour.clone();
    next.persons.retain(|p| &p.id != id);
    for p in next.persons.iter_mut() {
        if p.parent.as_ref() == Some(id) {
            p.parent = None;
        }
    }
    next.spendings
        .retain(|s| &s.from != id && s.kind != Kind::Planned);
    for s in next.spendings.iter_mut() {
        if let Split::Equally(to) | Split::ByWeight(to) = &mut s.split {
            to.retain(|p| p != id);
        }
        // A spending for nobody in particular is a spending for everybody.
        if matches!(&s.split, Split::Equally(to) | Split::ByWeight(to) if to.is_empty()) {
            s.split = Split::Everyone;
        }
    }
    next
}

// --- expenses -----------------------------------------------------------------------------

#[derive(serde::Deserialize, Default)]
pub struct Search {
    #[serde(default)]
    q: String,
}

pub async fn spend(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path(id): Path<String>,
    Query(search): Query<Search>,
) -> Response {
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}/spend"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };
    let tour = &loaded.tour;

    let all: Vec<&Spending> = tour
        .spendings
        .iter()
        .filter(|s| s.kind.counts(false))
        .collect();
    let needle = search.q.trim().to_lowercase();
    let shown: Vec<&&Spending> = all
        .iter()
        .filter(|s| {
            needle.is_empty()
                || s.description.to_lowercase().contains(&needle)
                || s.category.to_lowercase().contains(&needle)
                || name_of(tour, &s.from).to_lowercase().contains(&needle)
        })
        .collect();

    let body = if shown.is_empty() {
        format!(
            "<p>{}</p>",
            if needle.is_empty() {
                "No expenses yet."
            } else {
                "Nothing matches that search."
            }
        )
    } else {
        let total: Cents = shown.iter().map(|s| tour.amount_in_current(s)).sum();
        let rows: String = shown
            .iter()
            .map(|s| {
                format!(
                    r#"<tr><td>{date}</td><td>{what}</td><td>{by}</td><td>{for_whom}</td>
<td class="n">{amount}</td><td><a href="/t/{tour_id}/spend/edit/{sid}">edit</a></td></tr>"#,
                    date = esc(s.day().unwrap_or_default()),
                    what = esc(&s.description),
                    by = esc(&short(&name_of(tour, &s.from), 12)),
                    for_whom = esc(&for_whom(tour, s)),
                    amount = money(tour.amount_in_current(s)),
                    tour_id = esc(&loaded.id),
                    sid = esc(s.id.as_str()),
                )
            })
            .collect();
        format!(
            r#"<p class="muted">{count} of {total_count} &middot; {sum} {cur}</p>
<table><thead><tr><th>Date</th><th>What for</th><th>Paid by</th><th>For</th>
<th class="n">Amount</th><th></th></tr></thead>
<tbody>{rows}</tbody></table>"#,
            count = shown.len(),
            total_count = all.len(),
            sum = money(total),
            cur = esc(&loaded.currency),
        )
    };

    page(
        &format!("{} · expenses", tour.name),
        &format!(
            r#"{header}<h2>Expenses</h2>
<p><a href="/t/{id}/spend/edit">Add an expense</a></p>
<form method="get">
<label for="q">Search</label>
<input type="text" id="q" name="q" value="{q}" size="20" />
<button type="submit">Find</button>
{clear}
</form>
{body}"#,
            header = loaded.header("Expenses"),
            id = esc(&loaded.id),
            q = esc(&search.q),
            clear = if search.q.is_empty() {
                String::new()
            } else {
                format!("<a href=\"/t/{}/spend\">clear</a>", esc(&loaded.id))
            },
        ),
    )
    .into_response()
}

/// Who a spending was for, short enough for a narrow terminal.
fn for_whom(tour: &Tour, s: &Spending) -> String {
    match &s.split {
        Split::Everyone => "everyone".to_owned(),
        Split::Equally(to) | Split::ByWeight(to) => match to.len() {
            0 => "everyone".to_owned(),
            1 => short(&name_of(tour, &to[0]), 12),
            n => format!("{n} people"),
        },
    }
}

pub async fn spending_form(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path(path): Path<HashMap<String, String>>,
) -> Response {
    let id = path.get("id").cloned().unwrap_or_default();
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}/spend/edit"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };
    let spending = path.get("spending").and_then(|s| {
        loaded
            .tour
            .spendings
            .iter()
            .find(|x| x.id.as_str() == s && x.kind != Kind::Planned)
    });
    spending_page(&loaded, spending, None).into_response()
}

fn spending_page(
    loaded: &Loaded,
    spending: Option<&Spending>,
    error: Option<&str>,
) -> Html<String> {
    let tour = &loaded.tour;
    let is_new = spending.is_none();
    let title = if is_new {
        "New expense"
    } else {
        "Edit expense"
    };

    let amount = spending.map(|s| s.amount.0).unwrap_or(0);
    let description = spending.map(|s| s.description.as_str()).unwrap_or("");
    let category = spending.map(|s| s.category.as_str()).unwrap_or("");
    let from = spending.map(|s| s.from.clone());
    let to_all = spending.is_none_or(|s| s.split == Split::Everyone);
    let to: Vec<PersonId> = match spending.map(|s| &s.split) {
        Some(Split::Equally(to)) | Some(Split::ByWeight(to)) => to.clone(),
        _ => Vec::new(),
    };

    let payers: String = tour
        .persons
        .iter()
        .map(|p| {
            format!(
                "<option value=\"{id}\"{sel}>{name}</option>",
                id = esc(p.id.as_str()),
                sel = if from.as_ref() == Some(&p.id) {
                    " selected"
                } else {
                    ""
                },
                name = esc(&p.name),
            )
        })
        .collect();

    // Checkboxes rather than a multi-select: picking several options out of a
    // <select multiple> in a text browser is a fiddly business, a list of checkboxes is not.
    let recipients: String = tour
        .persons
        .iter()
        .map(|p| {
            format!(
                "<label><input type=\"checkbox\" name=\"ToGuid\" value=\"{id}\"{sel} /> {name}</label><br />",
                id = esc(p.id.as_str()),
                sel = if to.contains(&p.id) { " checked" } else { "" },
                name = esc(&p.name),
            )
        })
        .collect();

    let used: Vec<String> = {
        let mut cs: Vec<String> = tour
            .spendings
            .iter()
            .filter(|s| s.kind.counts(false))
            .map(|s| s.category.trim().to_owned())
            .filter(|c| !c.is_empty())
            .collect();
        cs.sort();
        cs.dedup();
        cs
    };
    let categories = if used.is_empty() {
        String::new()
    } else {
        format!(
            "<br /><span class=\"muted\">already used: {}</span>",
            esc(&used.join(", "))
        )
    };

    let action = match spending {
        Some(s) => format!("/t/{}/spend/edit/{}", loaded.id, s.id.as_str()),
        None => format!("/t/{}/spend/edit", loaded.id),
    };
    let delete = match spending {
        Some(s) => format!(
            r#"<form method="post" action="/t/{tour_id}/spend/delete/{sid}">
<button type="submit">Delete this expense</button></form>"#,
            tour_id = esc(&loaded.id),
            sid = esc(s.id.as_str()),
        ),
        None => String::new(),
    };
    let error = error
        .map(|e| format!("<p><strong>{}</strong></p>", esc(e)))
        .unwrap_or_default();

    page(
        title,
        &format!(
            r#"{header}<h2>{title}</h2>{error}
<form method="post" action="{action}">
<p><label for="amount">Amount</label><br />
<input type="number" id="amount" name="Amount" value="{amount}" size="12" /> {cur}</p>
<p><label for="desc">What for</label><br />
<input type="text" id="desc" name="Description" value="{description}" size="40" /></p>
<p><label for="from">Paid by</label>
<select id="from" name="FromGuid">{payers}</select></p>
<p><label><input type="checkbox" name="ToAll" value="true"{all} /> For everyone</label></p>
<fieldset><legend>…or only for these (ignored when "for everyone" is ticked)</legend>
{recipients}</fieldset>
<p><label for="type">Category</label>
<input type="text" id="type" name="Type" value="{category}" size="20" />{categories}</p>
<p><button type="submit">{button}</button>
<a href="/t/{tour_id}/spend">Cancel</a></p>
</form>
{delete}"#,
            header = loaded.header("Expenses"),
            cur = esc(&loaded.currency),
            description = esc(description),
            category = esc(category),
            all = if to_all { " checked" } else { "" },
            button = if is_new { "Add expense" } else { "Save" },
            tour_id = esc(&loaded.id),
        ),
    )
}

pub async fn spending_save(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path(path): Path<HashMap<String, String>>,
    body: String,
) -> Response {
    let id = path.get("id").cloned().unwrap_or_default();
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}/spend"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };

    let form = Fields::parse(&body);
    let amount = form.number("Amount").unwrap_or(0);
    let to_all = form.checked("ToAll");
    let to: Vec<PersonId> = form
        .all("ToGuid")
        .iter()
        .map(|g| PersonId::new(g.clone()))
        .collect();
    let from = PersonId::new(form.text("FromGuid").to_owned());

    let existing = path.get("spending").map(|s| SpendingId::new(s.clone()));
    let current = existing
        .as_ref()
        .and_then(|sid| loaded.tour.spendings.iter().find(|s| &s.id == sid));

    let problem = if amount == 0 {
        Some("How much was it?")
    } else if !to_all && to.is_empty() {
        Some("Who was it for?")
    } else if loaded.tour.person(&from).is_none() {
        Some("Who paid?")
    } else {
        None
    };
    if let Some(problem) = problem {
        return spending_page(&loaded, current, Some(problem)).into_response();
    }

    let split = if to_all {
        Split::Everyone
    } else {
        Split::Equally(to)
    };

    let mut next = (*loaded.tour).clone();
    next.spendings.retain(|s| s.kind != Kind::Planned);

    match existing.filter(|sid| next.spendings.iter().any(|s| &s.id == sid)) {
        Some(sid) => {
            if let Some(s) = next.spendings.iter_mut().find(|s| s.id == sid) {
                s.description = form.text("Description").to_owned();
                s.category = form.text("Type").to_owned();
                s.amount = Cents(amount);
                s.from = from;
                s.split = split;
            }
        }
        None => next.spendings.push(Spending {
            id: SpendingId::new(api_write::new_spending_id()),
            description: form.text("Description").to_owned(),
            category: form.text("Type").to_owned(),
            amount: Cents(amount),
            currency: loaded.tour.currency().clone(),
            from,
            split,
            remembered_split: None,
            kind: Kind::Real,
            extras: Default::default(),
        }),
    }

    match save(&state, &loaded.tour, next) {
        Ok(()) => Redirect::to(&format!("/t/{id}/spend")).into_response(),
        Err(response) => *response,
    }
}

pub async fn spending_delete(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path((id, spending)): Path<(String, String)>,
) -> Response {
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}/spend"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };

    let mut next = (*loaded.tour).clone();
    next.spendings
        .retain(|s| s.id.as_str() != spending && s.kind != Kind::Planned);

    match save(&state, &loaded.tour, next) {
        Ok(()) => Redirect::to(&format!("/t/{id}/spend")).into_response(),
        Err(response) => *response,
    }
}

// --- statistics ---------------------------------------------------------------------------

pub async fn stats(
    State(state): State<Shared>,
    Reader(auth): Reader,
    Path(id): Path<String>,
) -> Response {
    if auth.kind == "None" {
        return to_login(&format!("/t/{id}/stats"));
    }
    let loaded = match load(&state, &auth, &id) {
        Ok(l) => l,
        Err(response) => return *response,
    };
    let tour = &loaded.tour;

    let counted: Vec<&Spending> = tour
        .spendings
        .iter()
        .filter(|s| s.kind.counts(false) && !s.category.trim().is_empty())
        .collect();

    let total = loaded.total_spent;
    let people = tour.persons.len().max(1) as i64;
    let days = days_of(tour).max(1);
    let percent = |part: Cents| -> String {
        if total.0 == 0 {
            "0%".to_owned()
        } else {
            format!("{:.1}%", part.0 as f64 * 100.0 / total.0 as f64)
        }
    };

    let group = |key: &dyn Fn(&Spending) -> String| -> Vec<(String, usize, Cents)> {
        let mut groups: Vec<(String, usize, Cents)> = Vec::new();
        for s in &counted {
            let k = key(s);
            let amount = tour.amount_in_current(s);
            match groups.iter_mut().find(|(name, _, _)| name == &k) {
                Some(g) => {
                    g.1 += 1;
                    g.2 += amount;
                }
                None => groups.push((k, 1, amount)),
            }
        }
        groups.sort_by_key(|(_, _, sum)| -sum.0);
        groups
    };

    let table = |title: &str, rows: Vec<(String, usize, Cents)>| -> String {
        let body: String = rows
            .iter()
            .map(|(name, count, sum)| {
                format!(
                    r#"<tr><td>{name}</td><td class="n">{count}</td><td class="n">{share}</td><td class="n">{sum}</td></tr>"#,
                    name = esc(name),
                    share = percent(*sum),
                    sum = money(*sum),
                )
            })
            .collect();
        format!(
            r#"<h3>{title}</h3>
<table><thead><tr><th>{title}</th><th class="n">Entries</th><th class="n">Share</th>
<th class="n">Amount</th></tr></thead><tbody>{body}</tbody></table>"#
        )
    };

    let tables = if counted.is_empty() {
        "<p>Nothing to count yet &mdash; an expense needs a category to appear here.</p>".to_owned()
    } else {
        format!(
            "{}{}",
            table("By category", group(&|s: &Spending| s.category.clone())),
            table("By payer", group(&|s: &Spending| name_of(tour, &s.from))),
        )
    };

    page(
        &format!("{} · stats", tour.name),
        &format!(
            r#"{header}<h2>Statistics</h2>
<p><strong>{total}</strong> {cur} total &middot; <strong>{per_person}</strong> per person
&middot; <strong>{per_day}</strong> per person per day over {days} days</p>
{tables}"#,
            header = loaded.header("Stats"),
            total = money(total),
            cur = esc(&loaded.currency),
            per_person = money(Cents(total.0 / people)),
            per_day = money(Cents(total.0 / people / days)),
        ),
    )
    .into_response()
}
