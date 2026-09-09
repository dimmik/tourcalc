//! Prints the state the settlement sees at each round, for comparing against C#.
use tc_core::*;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "fixtures/zscph2y.tour.json".into());
    let json = std::fs::read_to_string(&path).unwrap();
    let tour = Tour::from_json(&json).unwrap();
    let b = calculate(&tour, Options { with_planned: true });
    let name = |id: &PersonId| tour.person(id).map(|p| p.name.clone()).unwrap_or_default();
    println!("=== балансы с учётом planned (то, с чего начинает подбор) ===");
    let mut rows: Vec<_> = b.per_person.iter().collect();
    rows.sort_by_key(|r| r.debt().0);
    for r in rows {
        println!(
            "{:>40}  spent {:>8}  recv {:>8}  debt {:>8}",
            name(&r.person),
            r.spent.0,
            r.received.0,
            r.debt().0
        );
    }
    println!("\n=== предложенные платежи ===");
    for t in suggest_settlement(&tour).unwrap() {
        println!(
            "{:>40} -> {:<40} {:>8}",
            name(&t.from),
            name(&t.to),
            t.amount.0
        );
    }
}
