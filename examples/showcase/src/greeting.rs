use rsc::component;

component! {
    pub Greeting
    schema {
        pub name: String;
    }
    impl {
        pub fn shout(&self) -> String {
            format!("{}!!!", self.name)
        }
    }
}
