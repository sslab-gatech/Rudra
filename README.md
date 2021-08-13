# Rudra

Rudra is a static analyzer to detect common undefined behaviors in Rust programs.

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
