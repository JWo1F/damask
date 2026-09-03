+++
eyebrow = "Compile-time components for Rust"
title = "Components with real slots."
closing = "The template is the markup"
lede = """
A component is a Rust struct and an HTML template that share a filename. The \
template declares as many named slots as it likes — with fallback content, \
forwarding, and a way to ask what the caller filled — and the struct never \
changes. Callers fill them the way they fill a web component's.\
"""

[[actions]]
label = "Start reading"
href = "/book/"
primary = true

[[actions]]
label = "Reference"
href = "/docs/"

[install]
code = '''
[dependencies]
damask = "{{version}}"
'''
note = "Rust 1.88 or newer. No build script, no configuration."

[weave]
rs_name = "card.rs"
rs = '''
use damask::Component;

#[derive(Component)]
pub struct Card {
    pub title: String,
    pub tone: Tone,
}
'''
dmk_name = "card.dmk"
dmk = '''
<article class={@tokens("card", self.tone.skin())}>
  <h3>{self.title}</h3>
  <slot/>
  <footer>
    <slot name="meta">Just now</slot>
  </footer>
</article>
'''
call_name = "page.dmk"
call = '''
<Card title="Damask {{version}}" tone={Tone::Note}>
  <p>Templates can await.</p>
</Card>
'''
out_name = "rendered"
out = '''
<article class="card card--note">
  <h3>Damask {{version}}</h3>
  <p>Templates can await.</p>
  <footer>Just now</footer>
</article>
'''

[[feature]]
title = "Slots, not children"
body = """
Most engines hand a component one anonymous block of content. A Damask template \
declares as many named slots as it wants: `<slot>` marks the place, its body is \
the fallback, and `slot="…"` on a direct child fills it. Several children may \
name the same slot, and a `<slot>` placed where a fill goes forwards a slot of \
your own straight through.\
"""
code = '''
<Frame title={self.heading.clone()}>
  <p>{self.body}</p>
  <span slot="footer">© {self.year}</span>
  <a slot="footer" href="/about">About</a>
</Frame>
'''

[[feature]]
title = "Two files, no wiring"
body = """
`button.rs` and `button.dmk`, side by side. Damask finds the template next to \
the struct, so there is no path attribute, no templates directory, no registry \
and no build script — and editing the template triggers a rebuild on its own. \
It needs Rust 1.88, which is when a macro first gained the ability to ask where \
it was written.\
"""
lang = "rust"
code = '''
// button.rs — paired with button.dmk
#[derive(Component)]
pub struct Button {
    pub label: String,
    pub disabled: bool,
}
'''

[[feature]]
title = "A missing prop is a compile error"
body = """
Component tags are checked like the struct literals they become. Rename a field \
and every call site that still passes the old one stops the build, with the \
field named in the message. A prop typed `Option<_>` may be left out, and \
arrives as `None`.\
"""
code = '''
<Button label="Save" disabled={self.locked}/>
<Button label="Cancel"/>   <!-- error: missing `disabled` -->
'''

[[feature]]
title = "Attributes that know they are HTML"
body = """
On an element, `attr={expr}` asks the value's type how to appear. A `bool` \
renders a bare attribute or nothing at all, and an `Option` renders nothing when \
it is `None` — because in HTML it is the *presence* of `disabled` that disables \
a control, and `disabled="false"` disables it too.\
"""
code = '''
<input title="row {self.n}"
       disabled={self.locked}
       placeholder={self.hint}/>
'''

[[feature]]
title = "Await where the data is"
body = """
A component whose data has not arrived yet writes `.await` in its markup, and \
the derive compiles that template to an async render instead — no attribute, \
nothing to configure, and no cost at all to the templates that don't. The \
future is `Send`, so a request handler can await one, and a plain sync child \
still drops into an awaiting parent unchanged.\
"""
code = '''
<ul class="feed">
  {#for deploy in self.store.recent(5).await}
    <li>{deploy.service}</li>
  {/for}
</ul>
'''

[[feature]]
title = "One component, any renderer"
body = """
`Renderer` owns the output buffer and the escaping policy, and components are \
compiled against `&mut dyn Renderer` — so one compiled component writes through \
the HTML renderer, a pretty-printing one, or one you wrote for a format Damask \
has never heard of. A child always writes through its *parent's* renderer, which \
is what makes escaping a property of the output rather than of the component.\
"""
lang = "rust"
code = '''
let mut r: Box<dyn Renderer> = Box::new(MyRenderer::new());
component.render_into(r.as_mut());
let out = r.finish();
'''

+++

There is no separate expression language to learn. A `{ … }` tag holds a Rust
block, so it sees whatever is in scope — `self`, an `impl` method beside the
struct, a `use` you wrote three lines up. Control flow is the Rust you already
know, spelled `{#if}` and `{#for}` so that a template still reads as markup.

Slots are matched by name when the page renders, and that is the price of
keeping them off the struct: a template may add or drop a `<slot>` without
touching a single field, and in exchange a misspelled slot name renders nothing
rather than failing the build. Fills are borrowed rather than owned, so content
passed from a caller stays on the caller's stack and can borrow the caller's
data without being boxed.
