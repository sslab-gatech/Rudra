# Rudra

Rudra is a static analyzer to detect common undefined behaviors in Rust programs.
It is capable of analyzing single Rust packages as well as all the packages on
crates.io.

Rudra and its associated paper will be presented at the
*Proceedings of the 28th ACM Symposium on Operating Systems Principles 2021*
(SOSP '21). ([preprint PDF](https://github.com/sslab-gatech/Rudra-Artifacts/raw/master/paper/sosp21-paper341.pdf))


## Usage

The easiest way to use Rudra is to use [Docker](https://www.docker.com/).

1. First, make sure your system has Docker and Python 3 installed.
2. Run `docker build . -t rudra:latest`.
3. Run `./setup_rudra_runner_home.py <directory>` and set `RUDRA_RUNNER_HOME` to that directory.
   Example: `./setup_rudra_runner_home.py ~/rudra-home && export RUDRA_RUNNER_HOME=$HOME/rudra-home`.
    * There are two scripts, `./setup_rudra_runner_home.py` and `./setup_rudra_runner_home_fixed.py`.
      In general, `./setup_rudra_runner_home.py` should be used unless you want to reproduce the result of the paper
      with a fixed cargo index.
4. Add `docker-helper` in Rudra repository to `$PATH`. Now you are ready to test Rudra!

For development, you might want to install Rudra on your host system.
See [DEV.md](DEV.md) for advanced usage and development guide.

### Run Rudra on a single project

```
docker-cargo-rudra <directory>
```

The log and report are printed to stderr by default.

## Bug Types Detected by Rudra

Rudra currently detects the following bug types.
For the full detail, please check our SOSP 2021 paper.

### Panic Safety (Unsafe code that can create memory-safety issues when panicked)

Detects when unsafe code may lead to memory safety issues if a user provided
closure or trait panics. For example, consider a function that dereferences a
pointer with `ptr::read`, duplicating its ownership and then calls a user
provided function `f`. This can lead to a double-free if the function `f`
panics.

See [this section of the Rustonomicon](https://doc.rust-lang.org/nomicon/exception-safety.html)
for more details.

```rust
while idx < len {
    let ch = unsafe { self.get_unchecked(idx..len).chars().next().unwrap() };
    let ch_len = ch.len_utf8();

    // Call to user provided predicate function f that can panic.
    if !f(ch) {
        del_bytes += ch_len;
    } else if del_bytes > 0 {
        unsafe {
            ptr::copy(
                self.vec.as_ptr().add(idx),
                self.vec.as_mut_ptr().add(idx - del_bytes),
                ch_len,
            );
        }
    }

    // Point idx to the next char
    idx += ch_len;
}
```

Example: [rust#78498](https://github.com/rust-lang/rust/issues/78498)

### Higher Order Invariant (Assumed properties about traits)

When code assumes certain properties about trait methods that aren't enforced,
such as expecting the `Borrow` trait to return the same reference on multiple
calls to `borrow`.

```rust
let mut g = Guard { len: buf.len(), buf }; 
// ...
  Ok(n) => g.len += n, 
```

Example: [rust#80894](https://github.com/rust-lang/rust/issues/80894)

### Send Sync Variance (Unrestricted Send or Sync on generic types)

This occurs when a type generic over `T` implements Send or Sync without having
correct bounds on `T`.

```rust
unsafe impl<T: ?Sized + Send, U: ?Sized> Send for MappedMutexGuard<'_, T, U> {} 
unsafe impl<T: ?Sized + Sync, U: ?Sized> Sync for MappedMutexGuard<'_, T, U> {} 
```

Example: [futures#2239](https://github.com/rust-lang/futures-rs/issues/2239)

## Bugs Found by Rudra

Rudra was ran on the entirety of crates.io state as of July 4th, 2020 as well
as the Rust standard library from `nightly-2020-08-26`. It managed to find 264
new memory safety issues across the Rust ecosystem which resulted in 78 CVEs.

The details of these bugs can be found in the [Rudra-PoC repo](https://github.com/sslab-gatech/Rudra-PoC).
