//! Getting the queue to the server, and resolving what happened while it waited.
//!
//! The interesting case is not "send the edits" - it is "somebody else sent theirs first".
//! Because the queue holds intentions rather than finished tours, that case has an answer
//! better than an apology: take their tour, replay ours on top, and try again. Both sets of
//! edits survive, and nobody is told to redo their work.
//!
//! Only a genuine collision - two people editing the same expense - can still lose
//! something, and it loses it the way any last-writer-wins system does. The alternative is
//! asking a person on a train to merge a JSON document, which is not an alternative.

use crate::api::{self, SaveError};
use crate::queue::{self, Operation};
use tc_core::Tour;

/// How the last attempt to reach the server went.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Everything is on the server.
    Synced,
    /// Nothing to send, and nothing was tried.
    Idle,
    /// What is on screen is this device's copy, and the server is being asked right now.
    /// True for a moment on every tour that has been opened here before - which is the
    /// point: the screen is drawn from what we have instead of waiting for the answer.
    Checking,
    /// This many edits are still waiting, because there is no network.
    Waiting(usize),
    /// Something went wrong that waiting will not fix.
    Failed(String),
}

/// The number of times a save may be beaten by somebody else before giving up.
///
/// Each round takes their tour and replays ours onto it, so a round only repeats if another
/// save landed in the meantime. Three is generous; the shape of the loop matters more than
/// the number, since without a limit two busy clients could keep each other going forever.
const MAX_ROUNDS: usize = 3;

/// Sends whatever is queued for this tour, and answers with the tour to build the screen
/// from.
///
/// What comes back is the **base** - the server's copy, or the last one this device saw -
/// with nothing from the queue applied. Applying the queue is the caller's job and happens
/// in exactly one place ([`queue::with_pending`]); doing it here as well is how an edit
/// once appeared twice on screen.
pub async fn push(tour_id: &str) -> (Option<Tour>, Status) {
    let ops = queue::pending(tour_id);

    // Nothing of ours to send: just take what the server has.
    if ops.is_empty() {
        return fetch(tour_id).await;
    }

    // One send per tour at a time. Two of them - the open page and the app noticing the
    // network is back - would each read the server, replay the same queue onto what the
    // other had just written, and leave an expense recorded twice. The second caller takes
    // what this device has and says the answer is on its way.
    let Some(_sending) = Sending::of(tour_id) else {
        return (queue::cached(tour_id), Status::Checking);
    };

    for _ in 0..MAX_ROUNDS {
        // Always start from the freshest server copy: that is what makes a conflict
        // recoverable instead of fatal.
        let server = match api::tour(tour_id).await {
            Ok(t) => t,
            Err(e) if api::looks_offline(&e) => {
                return (queue::cached(tour_id), Status::Waiting(ops.len()));
            }
            Err(e) => {
                refused(tour_id, &e);
                return (queue::cached(tour_id), Status::Failed(e));
            }
        };

        let (next, lost) = replay_saying_what_was_lost(&ops, &server);

        match api::save_tour(&next).await {
            Ok(()) => {
                queue::not_refused(tour_id);
                queue::set_lost(tour_id, Some(&lost));
                queue::set_pending(tour_id, &[]);
                // Read back rather than assume: the server assigns the new state id, and
                // the next save has to carry it.
                return match api::tour(tour_id).await {
                    Ok(t) => {
                        queue::cache(&t);
                        (Some(t), Status::Synced)
                    }
                    Err(_) => (Some(next), Status::Synced),
                };
            }
            // Somebody was quicker. Their tour is now the base; go round again.
            Err(SaveError::Conflict) => continue,
            // The read worked and the write did not, so the freshest base is the one just
            // read - not the older copy in the cache.
            Err(SaveError::Offline(_)) => {
                queue::cache(&server);
                return (Some(server), Status::Waiting(ops.len()));
            }
            Err(SaveError::Other(e)) => {
                refused(tour_id, &e);
                return (Some(server), Status::Failed(e));
            }
        }
    }

    (
        queue::cached(tour_id),
        Status::Failed(format!(
            "Could not save: the tour kept changing underneath ({MAX_ROUNDS} tries)."
        )),
    )
}

/// Notes that the server said no to this queue - unless what it said no to was the login,
/// which is not the queue's fault: signing in again sends it as it is.
fn refused(tour_id: &str, why: &str) {
    if api::signed_in() {
        queue::was_refused(tour_id, why);
    }
}

thread_local! {
    /// The tours a send is in flight for. A thread local because this is a browser: one
    /// thread, and the whole app is inside it.
    static SENDING: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

/// Marks a tour as being sent, and unmarks it however the send ends.
struct Sending(String);

impl Sending {
    fn of(tour: &str) -> Option<Sending> {
        SENDING.with(|busy| busy.borrow_mut().insert(tour.to_owned()))
            .then(|| Sending(tour.to_owned()))
    }
}

impl Drop for Sending {
    fn drop(&mut self) {
        SENDING.with(|busy| busy.borrow_mut().remove(&self.0));
    }
}

/// What the server has, and nothing sent - whatever is queued stays queued.
///
/// For a load nobody asked to send anything: the page noticing somebody else's change
/// fetches this way, so that it can never become a second send of a queue that a save is
/// already sending (see `others`).
pub async fn fetch(tour_id: &str) -> (Option<Tour>, Status) {
    let waiting = queue::pending(tour_id).len();
    match api::tour(tour_id).await {
        Ok(t) => {
            queue::cache(&t);
            (Some(t), Status::Idle)
        }
        Err(e) if api::looks_offline(&e) => (queue::cached(tour_id), Status::Waiting(waiting)),
        Err(e) => (queue::cached(tour_id), Status::Failed(e)),
    }
}

/// The tour with every queued operation carried out, in the order they were made - and the
/// edits that came to nothing because somebody else had deleted what they edited.
///
/// Checked against the tour as it stands when each edit's turn comes, not against the
/// server's: an edit of an expense added earlier in the same queue is not lost.
fn replay_saying_what_was_lost(ops: &[Operation], base: &Tour) -> (Tour, Vec<String>) {
    let mut lost = Vec::new();
    let tour = ops.iter().fold(base.clone(), |acc, op| {
        lost.extend(op.lost_on(&acc));
        op.apply(&acc)
    });
    (tour, lost)
}

/// Records an edit and tries to send it.
///
/// The edit is written down first and sent second, which is the whole difference between an
/// app that works on a train and one that does not.
pub async fn record(tour_id: &str, op: Operation) -> (Option<Tour>, Status) {
    queue::push(tour_id, op);
    push(tour_id).await
}
