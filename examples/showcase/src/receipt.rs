use damask::{Component, Trusted, tag};

/// Markup built in Rust, spliced into a template beside data that is not.
///
/// The two fields go through the same `{ … }` and come out differently: the
/// note is a string somebody typed, so it is escaped, and the badge is markup,
/// so it is written as it stands. Neither the template nor the caller says
/// which — the type does.
///
/// The `row` snippet is the same point made where it is hardest: its parameter
/// carries no type at all, so what it holds is only known at each
/// `{@render row(…)}`. It is rendered once with markup and once with a string,
/// and each comes out the way its own type says.
#[derive(Component)]
pub struct Receipt {
    pub note: String,
    pub badge: Trusted,
    pub lines: Vec<Trusted>,
}

impl Receipt {
    /// A helper of the shape `tag!` exists for: markup a template would be a
    /// clumsy way to write, returned as a value a template can splice.
    pub fn badge(label: &str, urgent: bool) -> Trusted {
        tag!(span, class: @tokens("badge", urgent.then_some("badge--urgent")), label)
    }

    pub fn line(name: &str, amount: f64) -> Trusted {
        tag!(span, (tag!(b, name), " — ", amount))
    }
}
