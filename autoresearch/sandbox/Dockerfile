# Sandbox Docker image for isolated rule experiments
# 
# Build: docker build -t cognicode-sandbox:latest .
# Run:   docker run --rm cognicode-sandbox RULE_ID GIT_REF CHANGE_SCRIPT

FROM docker.io/library/rust:1.92-slim-bookworm

LABEL description="CogniCode self-evolving rule sandbox"
LABEL version="0.1.0"

# ═══════════════════════════════════════════════════════════════════
# System dependencies
# ═══════════════════════════════════════════════════════════════════

RUN apt-get update && apt-get install -y --no-install-recommends \
    git curl ca-certificates \
    build-essential pkg-config libssl-dev \
    python3 python3-pip python3-venv \
    nodejs npm \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# ═══════════════════════════════════════════════════════════════════
# Rust toolchain
# ═══════════════════════════════════════════════════════════════════

RUN rustup component add clippy rustfmt
RUN rustup target add wasm32-unknown-unknown

# ═══════════════════════════════════════════════════════════════════
# External static analysis tools
# ═══════════════════════════════════════════════════════════════════

# Python: Ruff (fast linter)
RUN pip3 install --break-system-packages ruff

# JavaScript: ESLint
RUN npm install -g eslint @typescript-eslint/parser @typescript-eslint/eslint-plugin

# SonarQube Scanner (optional, heavy — uncomment if needed)
# RUN curl -sSLo /tmp/sonar-scanner.zip \
#     https://binaries.sonarsource.com/Distribution/sonar-scanner-cli/sonar-scanner-cli-5.0.1.3006-linux.zip \
#     && unzip -q /tmp/sonar-scanner.zip -d /opt \
#     && ln -s /opt/sonar-scanner-*/bin/sonar-scanner /usr/local/bin/sonar-scanner \
#     && rm /tmp/sonar-scanner.zip

# Go: staticcheck (for Go corpus evaluation, optional)
# RUN curl -sSL https://github.com/dominikh/go-tools/releases/download/2024.1/staticcheck_linux_amd64.tar.gz \
#     | tar -xz -C /usr/local/bin staticcheck/staticcheck --strip-components=1

# ═══════════════════════════════════════════════════════════════════
# Workspace setup
# ═══════════════════════════════════════════════════════════════════

WORKDIR /workspace

# Entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Default: no network access (security)
# Override with --network=host if external tools need network

ENTRYPOINT ["/entrypoint.sh"]
CMD ["--help"]
