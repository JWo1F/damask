//! End-to-end behavior of the async example components: a template whose own
//! `.dmk` contains `.await` renders through `AsyncComponent::render_async`
//! rather than `Component::render`, composes with a plain sync child, and
//! still supports snippets and `<slot>` fallbacks.

use damask::AsyncComponent;
use damask_showcase::async_greeting::AsyncGreeting;
use damask_showcase::async_panel::AsyncPanel;

/// A single-threaded, no-IO executor: no runtime dependency, and every future
/// here resolves without ever really suspending, so busy-polling with a
/// no-op waker is enough.
fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

#[test]
fn a_direct_await_renders_through_render_async() {
    let g = AsyncGreeting {
        name: "<Ada>".into(),
    };
    assert_eq!(block_on(g.render_async()), "Hello &lt;Ada&gt;!");
}

#[test]
fn an_async_template_composes_a_sync_child_a_snippet_and_a_slot_fallback() {
    let p = AsyncPanel { label: "ok".into() };
    let out = block_on(p.render_async());
    assert!(out.contains("<button>fixed</button>"), "{out}");
    assert!(out.contains("<p>OK</p>"), "{out}");
    assert!(out.contains("<footer>ok</footer>"), "{out}");
}
