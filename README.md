# Rudra

Rudra is a static analyzer to detect common undefined behaviors in Rust programs.
It is capable of analyzing single Rust packages as well as all the packages on
crates.io.

Rudra and its associated paper were presented at the
*Proceedings of the 28th ACM Symposium on Operating Systems Principles 2021*
(SOSP '21). ([PDF](https://github.com/sslab-gatech/Rudra-Artifacts/raw/master/paper/sosp21-paper341.pdf))


TODO: briefly explain bug patterns, add links to our paper and PoC repositories.

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

## Bugs Found by Rudra

Rudra was ran on the entirety of crates.io state as of July 4th, 2020 as well
as the Rust standard library from `nightly-2020-08-26`. It managed to find 264
new memory safety issues across the Rust ecosystem which resulted in 78 CVEs.

The details of these bugs can be found in the [Rudra-PoC repo](https://github.com/sslab-gatech/Rudra-PoC).
