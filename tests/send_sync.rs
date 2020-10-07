/// Example for testing send/sync checker 
/// modified from source: https://github.com/slide-rs/atom/commit/dc096ef61a7cf69ce1a833a277b1c2a7c726c129
use std::fmt::Display;

struct Atom1<P>(P);
unsafe impl<P: Clone> Send for Atom1<P> {}
unsafe impl<P: Copy + Sync> Sync for Atom1<P> {}

struct Atom2<P>(P);
unsafe impl<P> Sync for Atom2<P> {}

struct Atom3<P>(P);
unsafe impl<P> Send for Atom3<P>
where 
    P: Copy + Send
{ }

unsafe impl<P> Sync for Atom3<P>
where
    P: Display + Clone
{ }

struct Atom4<P>(P);
unsafe impl<P> Sync for Atom4<P>
where
    P: Sync
{}
