use damask::Component;
use damask_showcase::button::Button;
use damask_showcase::card::Card;
use damask_showcase::greeting::Greeting;
use damask_showcase::list::List;
use damask_showcase::menu::Menu;

fn main() {
    let greeting = Greeting { name: "Ada".into() };
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
