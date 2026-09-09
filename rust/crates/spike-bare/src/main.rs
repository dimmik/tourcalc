use tc_core::{calculate, suggest_settlement, Options, PersonId, Tour};

const FIXTURE: &str = include_str!("../../../fixtures/zscph2y.tour.json");

fn main() {
    let tour = Tour::from_json(FIXTURE).expect("fixture parses");
    let balances = calculate(&tour, Options::default());
    let transfers = suggest_settlement(&tour).expect("converges");

    let name_of = |id: &PersonId| {
        tour.person(id)
            .map(|p| p.name.as_str())
            .unwrap_or("n/a")
            .to_owned()
    };

    let doc = web_sys::window().unwrap().document().unwrap();
    let body = doc.body().unwrap();

    let head = doc.create_element("h1").unwrap();
    head.set_text_content(Some(&tour.name));
    body.append_child(&head).unwrap();

    let list = doc.create_element("ul").unwrap();
    for t in &transfers {
        let li = doc.create_element("li").unwrap();
        li.set_text_content(Some(&format!(
            "{} → {}: {}",
            name_of(&t.from),
            name_of(&t.to),
            t.amount
        )));
        list.append_child(&li).unwrap();
    }
    for b in &balances.per_person {
        let li = doc.create_element("li").unwrap();
        li.set_text_content(Some(&format!(
            "{}: spent {} share {} debt {}",
            name_of(&b.person),
            b.spent,
            b.received,
            b.debt()
        )));
        list.append_child(&li).unwrap();
    }
    body.append_child(&list).unwrap();
}
