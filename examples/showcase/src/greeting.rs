use damask::Component;

#[derive(Component)]
pub struct Greeting {
    pub name: String,
}

impl Greeting {
    pub fn shout(&self) -> String {
        format!("{}!!!", self.name)
    }
}
