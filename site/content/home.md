+++
eyebrow = "Compile-time components for Rust"
title = "Templates woven into the binary."
closing = "Written the way you would write the HTML"
lede = """
A Damask component is a Rust struct paired with an HTML template. The derive \
turns the template into a `render` method at build time, so rendering a page is \
plain, allocation-light Rust — no runtime engine, no template cache, no \
interpreter between your data and the response.\
"""

[[actions]]
label = "Start reading"
href = "/book/"
primary = true

[[actions]]
label = "Reference"
href = "/docs/"

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
<article class=["card", self.tone.skin()]>
  <h3>{self.title}</h3>
  <slot/>
</article>
'''
out_name = "rendered"
out = '''
<article class="card card--warn">
  <h3>Disk almost full</h3>
  <p>3% left on /dev/sda1.</p>
</article>
'''

[[feature]]
title = "Two files, one component"
body = """
The struct's fields are the props; the template beside it is the markup. Damask \
finds the pair by name, so there is no build script, no macro argument and no \
registry to keep in sync — editing the template triggers a rebuild on its own.\
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
field named in the message.\
"""
code = '''
<Button label="Save" disabled={self.locked}/>
<Button label="Cancel"/>   <!-- error: missing `disabled` -->
'''

[[feature]]
title = "Escaping you cannot forget"
body = """
`{ … }` escapes. Raw output has to be asked for by name, with `{@html … }`, so \
the unsafe case is the one that is visible in review — and the `Renderer` trait \
is where a project changes that policy wholesale.\
"""
code = '''
{self.name}          <!-- <b> becomes &lt;b&gt; -->
{@html self.body}    <!-- verbatim, and it looks it -->
'''

[[feature]]
title = "Slots, without the struct knowing"
body = """
A template declares as many slots as it likes and the struct never changes. \
`<slot>` marks the place, `slot="…"` fills it — as web components do — and \
content passed from a caller stays on the caller's stack, borrowed, not boxed.\
"""
code = '''
<Frame title={self.heading.clone()}>
  <p>{self.body}</p>
  <span slot="footer">© {self.year}</span>
  <a slot="footer" href="/about">About</a>
</Frame>
'''

+++

There is no separate expression language to learn. A `{ … }` tag holds a Rust
block, so it sees whatever is in scope — `self`, an `impl` method beside the
struct, a `use` you wrote three lines up. Control flow is the Rust you already
know, spelled `{#if}` and `{#each}` so that a template still reads as markup.

What comes out the other end is a `render` method: a run of `write_raw` calls
over string literals the compiler already knows, with your values escaped into
the gaps. Nothing parses a template at runtime, because by then there is no
template left to parse.
