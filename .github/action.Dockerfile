# Pulling latest release from GitHub Packages
# Not using the default Dockerfile because that will cause
# users to rebuild the container every time its run
FROM ghcr.io/sslab-gatech/rudra:master

ENTRYPOINT ["cargo", "rudra"]

