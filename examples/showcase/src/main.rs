use rsc::Component;
use rsc_showcase::button::Button;
use rsc_showcase::card::Card;
use rsc_showcase::greeting::Greeting;
use rsc_showcase::list::List;
use rsc_showcase::menu::Menu;

fn main() {
    let greeting = Greeting {
        name: "Ada".into(),
    };
    println!("{}", greeting.render());

    let card = Card {
        button: Button {
            label: "Click <me>".into(),
        },
    };
    println!("{}", card.render());

    let list = List {
        items: vec!["one".into(), "two".into(), "three".into()],
    };
    println!("{}", list.render());

    let menu = Menu {
        labels: vec!["Home".into(), "About".into()],
    };
    println!("{}", menu.render());
}
