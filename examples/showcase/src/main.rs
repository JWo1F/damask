mod greeting;

use greeting::Greeting;
use rsc::Component;

fn main() {
    let g = Greeting {
        name: "Ada".into(),
    };
    println!("{}", g.render());
    println!("{}", g.shout());
}

#[cfg(test)]
mod tests {
    use super::greeting::Greeting;
    use rsc::Component;

    #[test]
    fn renders_and_escapes() {
        let g = Greeting {
            name: "<Ada>".into(),
        };
        assert_eq!(g.render(), "Hello &lt;Ada&gt;!");
    }

    #[test]
    fn inherent_methods_are_kept() {
        let g = Greeting { name: "Ada".into() };
        assert_eq!(g.shout(), "Ada!!!");
    }
}
