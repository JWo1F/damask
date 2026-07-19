//! `<Notice detail={None}/>` — `title` has no `Option` to say it may be left
//! out, so the builder never reaches `__rsc_build`.

use rsc_showcase::notice::Notice;

fn main() {
    let _ = Notice::__rsc_props().detail(None).__rsc_build();
}
