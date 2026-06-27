use rsc::Component;

#[derive(Component)]
#[template(bogus = "x")]
struct Bad {
    x: u32,
}

fn main() {}
