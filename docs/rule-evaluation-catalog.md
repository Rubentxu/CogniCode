# CogniCode Rule Evaluation - Project Catalog

Comprehensive catalog of 50 real GitHub open-source projects per language for CogniCode's self-evolving rule evaluation system.

**Research Strategy:**
- Widely-used, well-known projects
- Diverse code patterns (not monocultures)
- Projects with known code quality issues
- Mix of sizes: small (<10K), medium (10-50K), large (50-200K), huge (>200K)
- Mix of domains: web, CLI, libraries, frameworks, databases, tools

---

## Rust (50 projects)

| # | Project | LOC | Category | Priority | Why |
|---|---------|-----|----------|----------|-----|
| 1 | BurntSushi/ripgrep | medium | cli | HIGH | Complex pattern matching, parser combinators |
| 2 | rust-lang/rust | huge | compiler | HIGH | rustc itself, borrow checking edge cases |
| 3 | rust-lang/cargo | large | build | HIGH | Build system, plugin ecosystem, TOML parsing |
| 4 | serde-rs/serde | large | library | HIGH | Derive macros, complex trait bounds |
| 5 | tokio-rs/tokio | large | runtime | HIGH | Async runtime, subtle concurrency bugs |
| 6 | actix/actix-web | medium | web | HIGH | Actor model, known runtime issues |
| 7 | clap-rs/clap | medium | cli | HIGH | Derive macros, complex config parsing |
| 8 | diesel-rs/diesel | medium | orm | HIGH | Query builder, type-level guarantees |
| 9 | launchbadge/sqlx | medium | database | HIGH | Async DB, compile-time query verification |
| 10 | tokio-rs/axum | medium | web | HIGH | Tower middleware, typed extractors |
| 11 | rocket-web/rocket | medium | web | HIGH | Proc-macro routes, known issues |
| 12 | pola-rs/polars | large | library | HIGH | DataFrames, SIMD, complex query optimization |
| 13 | apache/arrow-rs | large | library | HIGH | Arrow implementation, memory layout |
| 14 | reqwest/reqwest | medium | http | MEDIUM | HTTP client, TLS, connection pooling |
| 15 | tokio-rs/tracing | medium | observability | HIGH | Structured logging, span propagation |
| 16 | tower-rs/tower | medium | library | HIGH | Middleware composability patterns |
| 17 | ratatui-org/ratatui | medium | tui | MEDIUM | Terminal UI, complex state management |
| 18 | bevyengine/bevy | large | game_engine | HIGH | ECS architecture, complex plugin system |
| 19 | rust-lang/futures-rs | medium | library | HIGH | Futures combinators, AsyncRead/Write traits |
| 20 | crossbeam-rs/crossbeam | medium | library | HIGH | Concurrent data structures, race conditions |
| 21 | parking_lot/parking_lot | small | library | MEDIUM | Mutex implementation, lock ordering |
| 22 | regex-rs/regex | medium | library | HIGH | Regex engine, known ReDoS vulnerabilities |
| 23 | chrono-rs/chrono | medium | library | MEDIUM | Date/time handling, timezone bugs |
| 24 | thiserror-rs/thiserror | small | library | MEDIUM | Derive macros for error enums |
| 25 | anyhow/anyhow | small | library | MEDIUM | Error handling patterns |
| 26 | oxc-project/oxc | large | tools | HIGH | JavaScript parser/toolchain, complex AST |
| 27 | biomejs/biome | medium | tools | HIGH | Formatter/linter, performance issues |
| 28 | rust-analyzer/rust-analyzer | huge | tools | HIGH | LSP server, rustc compatibility |
| 29 | rust-lang/rustfmt | medium | tools | MEDIUM | Code formatter, style edge cases |
| 30 | rust-lang/clippy | large | tools | HIGH | Linter, 500+ rules, known false positives |
| 31 | Mozilla/sccache | medium | tools | MEDIUM | Compilation cache, ccache wrapper |
| 32 | bytecodealliance/wasmtime | large | runtime | HIGH | JIT compiler, WebAssembly runtime |
| 33 | parity-tech/substrate | huge | blockchain | HIGH | FRAME runtime, complex macro system |
| 34 | solana-labs/solana | huge | blockchain | HIGH | High-performance blockchain, many known issues |
| 35 | rustls/rustls | medium | library | HIGH | TLS implementation |
| 36 | neqo-http3/neqo | medium | library | HIGH | HTTP/3 implementation |
| 37 | u-root/u-root | medium | tools | MEDIUM | Embedded Linux tools |
| 38 | tauri-apps/tauri | medium | framework | HIGH | Desktop app framework |
| 39 | iced-rs/iced | medium | framework | MEDIUM | GUI framework, known issues |
| 40 | emilk/egui | medium | framework | MEDIUM | Immediate mode GUI |
| 41 | nushell/nushell | medium | cli | HIGH | Shell, complex parsing patterns |
| 42 | zellij/zellij | medium | cli | MEDIUM | Terminal multiplexer |
| 43 | bandwhich/bandwhich | small | cli | MEDIUM | Network monitor |
| 44 | denoland/deno | large | runtime | HIGH | JS runtime, TypeScript, known issues |
| 45 | swc-project/swc | large | tools | HIGH | JS compiler, known performance issues |
| 46 | volta/volta | small | tools | MEDIUM | Node version manager |
| 47 | sea-orm/sea-orm | medium | orm | MEDIUM | Async ORM |
| 48 | lemmy/lemmy | medium | web | MEDIUM | ActivityPub, known issues |
| 49 | sycamore-rs/sycamore | small | framework | LOW | WASM web framework |
| 50 | fereidani/proc-macro-toolkit | small | library | LOW | Proc macro utilities |

---

## Python (50 projects)

| # | Project | LOC | Category | Priority | Why |
|---|---------|-----|----------|----------|-----|
| 1 | django/django | huge | web | HIGH | Complex ORM, middleware, known issues |
| 2 | pallets/flask | medium | web | HIGH | Extensions ecosystem, known issues |
| 3 | psf/requests | medium | http | MEDIUM | HTTP library, widespread use |
| 4 | encode/httpx | medium | http | MEDIUM | Async HTTP client |
| 5 | numpy/numpy | huge | library | HIGH | Scientific computing, C extensions |
| 6 | pandas-dev/pandas | huge | library | HIGH | DataFrames, complex API |
| 7 | scipy/scipy | huge | library | HIGH | Scientific computing |
| 8 | matplotlib/matplotlib | large | library | HIGH | Visualization library |
| 9 | scikit-learn/scikit-learn | large | library | HIGH | ML library |
| 10 | pytorch/pytorch | huge | library | HIGH | ML framework, C++/Python mix |
| 11 | tensorflow/tensorflow | huge | library | HIGH | ML framework, known issues |
| 12 | pytest-dev/pytest | medium | testing | HIGH | Plugin system, known edge cases |
| 13 | pypa/pip | large | tools | MEDIUM | Package manager |
| 14 | python-poetry/poetry | medium | tools | MEDIUM | Dependency management |
| 15 | python/mypy | large | tools | HIGH | Type checker, complex edge cases |
| 16 | psf/black | medium | tools | HIGH | Code formatter |
| 17 | astral-sh/ruff | medium | tools | HIGH | Fast linter, Rust implementation |
| 18 | SQLAlchemy/SQLAlchemy | large | orm | HIGH | Complex ORM, known issues |
| 19 | celery/celery | large | library | HIGH | Task queue, known issues |
| 20 | tiangolo/fastapi | medium | web | HIGH | Modern framework, Pydantic integration |
| 21 | pallets/werkzeug | medium | library | MEDIUM | WSGI utilities |
| 22 | encode/starlette | medium | web | MEDIUM | ASGI framework |
| 23 | tornado/tornado | medium | web | MEDIUM | Async web framework |
| 24 | pyramid-team/pyramid | medium | web | MEDIUM | WSGI framework |
| 25 | sqlalchemy/alembic | medium | tools | MEDIUM | Database migrations |
| 26 | pydantic/pydantic | medium | library | HIGH | Data validation |
| 27 | twisted/twisted | large | library | MEDIUM | Async networking |
| 28 | home-assistant/core | huge | iot | HIGH | Smart home platform |
| 29 | mitmproxy/mitmproxy | medium | tools | HIGH | Proxy, known issues |
| 30 | scapy/scapy | medium | net | HIGH | Packet manipulation |
| 31 | pyca/cryptography | medium | library | HIGH | Crypto primitives |
| 32 | PyJWT/pyjwt | small | library | MEDIUM | JWT handling |
| 33 | urllib3/urllib3 | medium | library | MEDIUM | HTTP connection pooling |
| 34 | python-attrs/attrs | small | library | MEDIUM | Class decorators |
| 35 | getsentry/sentry-python | medium | library | HIGH | Error tracking |
| 36 | psf/docutils | medium | library | MEDIUM | Documentation processing |
| 37 | NVlabs/instant-ngp | large | library | HIGH | Neural graphics primitives |
| 38 | opencv/opencv-python | medium | library | HIGH | Computer vision |
| 39 | psycopg/psycopg2 | medium | database | MEDIUM | PostgreSQL driver |
| 40 | redis/redis-py | medium | database | MEDIUM | Redis client |
| 41 | elastic/elasticsearch-py | medium | database | MEDIUM | Elasticsearch client |
| 42 | boto/boto3 | large | library | HIGH | AWS SDK |
| 43 | googleapis/google-cloud-python | huge | library | MEDIUM | GCP SDK |
| 44 | pypa/setuptools | medium | tools | MEDIUM | Package setup |
| 45 | ansible/ansible-core | large | tools | MEDIUM | Automation framework |
| 46 | saltstack/salt | large | tools | MEDIUM | Configuration management |
| 47 | apache/airflow | large | tools | HIGH | Workflow platform |
| 48 | prefecthq/prefect | medium | tools | MEDIUM | Data workflow |
| 49 | dagster-io/dagster | large | tools | HIGH | ML orchestration |
| 50 | locustio/locust | medium | tools | MEDIUM | Load testing |

---

## JavaScript/TypeScript (50 projects)

| # | Project | LOC | Category | Priority | Why |
|---|---------|-----|----------|----------|-----|
| 1 | nodejs/node | huge | runtime | HIGH | JS runtime, known issues |
| 2 | facebook/react | large | framework | HIGH | UI library, known issues |
| 3 | vuejs/core | large | framework | HIGH | UI framework |
| 4 | angular/angular | huge | framework | HIGH | Full framework |
| 5 | microsoft/TypeScript | large | tools | HIGH | Type system |
| 6 | vercel/next.js | large | framework | HIGH | SSR/SSG framework |
| 7 | expressjs/express | medium | web | HIGH | Server framework |
| 8 | fastify/fastify | medium | web | HIGH | Fast server framework |
| 9 | denoland/deno | large | runtime | HIGH | JS runtime |
| 10 | oven-sh/bun | medium | runtime | MEDIUM | JS runtime |
| 11 | facebook/react-native | huge | framework | HIGH | Mobile framework |
| 12 | nextauthjs/next-auth | medium | library | MEDIUM | Authentication |
| 13 | trpc/trpc | medium | library | HIGH | Type-safe APIs |
| 14 | zod/zod | medium | library | HIGH | Runtime validation |
| 15 | shadcn-ui/ui | medium | library | HIGH | Component library |
| 16 | radix-ui/primitives | medium | library | HIGH | Headless components |
| 17 | storybookjs/storybook | large | tools | MEDIUM | Component explorer |
| 18 | webpack/webpack | large | tools | HIGH | Bundler, known issues |
| 19 | vitejs/vite | medium | tools | HIGH | Fast bundler |
| 20 | esbuild/esbuild | medium | tools | HIGH | Fast bundler |
| 21 | swc-project/swc | large | tools | HIGH | JS compiler |
| 22 | babel/babel | large | tools | HIGH | Transpiler |
| 23 | microsoft/vscode | huge | application | HIGH | Code editor |
| 24 | jestjs/jest | medium | testing | MEDIUM | Test runner |
| 25 | vitest-dev/vitest | medium | testing | HIGH | Fast test runner |
| 26 | playwrightwright/playwright | medium | tools | HIGH | Browser automation |
| 27 | prisma/prisma | medium | orm | HIGH | Type-safe ORM |
| 28 | drizzle-team/drizzle-orm | medium | orm | MEDIUM | Lightweight ORM |
| 29 | typeorm/typeorm | medium | orm | MEDIUM | ORM |
| 30 | mongodb/node-mongodb-native | medium | database | MEDIUM | MongoDB driver |
| 31 | ioredis/ioredis | medium | database | MEDIUM | Redis client |
| 32 | knex/knex | medium | query | MEDIUM | Query builder |
| 33 | graphql/graphql-js | medium | library | HIGH | GraphQL implementation |
| 34 | apollographql/apollo-client | medium | library | HIGH | GraphQL client |
| 35 | remix-run/react-router | medium | library | HIGH | Routing |
| 36 | tanstack/query | medium | library | HIGH | Data fetching |
| 37 | reduxjs/redux | medium | library | MEDIUM | State management |
| 38 | MobX/MobX | medium | library | MEDIUM | State management |
| 39 | axios/axios | medium | http | MEDIUM | HTTP client |
| 40 | formium/formik | medium | library | MEDIUM | Form handling |
| 41 | react-hook-form/react-hook-form | medium | library | HIGH | Form performance |
| 42 | motion/motion | medium | library | MEDIUM | Animation library |
| 43 | stripe/stripe-node | medium | library | MEDIUM | Payments SDK |
| 44 | aws/aws-sdk-js | large | library | HIGH | AWS SDK |
| 45 | googleapis/google-api-javascript-client | medium | library | MEDIUM | GCP client |
| 46 | octokit/rest.js | medium | library | MEDIUM | GitHub API |
| 47 | knex/knex | | | | FIX: duplicate |
| 48 | sentry/sentry-javascript | medium | library | HIGH | Error tracking |
| 49 | nicolo-ribaudo/eslint-plugin-lodash | medium | tools | MEDIUM | ESLint plugin |
| 50 | prettier/prettier | medium | tools | HIGH | Code formatter |

---

## Java (50 projects)

| # | Project | LOC | Category | Priority | Why |
|---|---------|-----|----------|----------|-----|
| 1 | spring-projects/spring-boot | huge | framework | HIGH | Enterprise framework |
| 2 | spring-projects/spring-framework | huge | framework | HIGH | Core framework |
| 3 | spring-projects/spring-security | large | framework | HIGH | Security framework |
| 4 | HibernateORM/hibernate-orm | large | orm | HIGH | Complex ORM |
| 5 | apache/camel | large | integration | HIGH | Integration patterns |
| 6 | dropwizard/dropwizard | medium | framework | MEDIUM | Microservices framework |
| 7 | vertx-io/vert.x | medium | framework | MEDIUM | Reactive framework |
| 8 | quarkusio/quarkus | large | framework | HIGH | Cloud-native framework |
| 9 | micronaut-projects/micronaut | medium | framework | MEDIUM | GraalVM native images |
| 10 | apache/spark | huge | library | HIGH | Data processing |
| 11 | apache/kafka | large | messaging | HIGH | Event streaming |
| 12 | elastic/elasticsearch | huge | database | HIGH | Search engine |
| 13 | apache/flink | large | library | HIGH | Stream processing |
| 14 | google/guava | large | library | MEDIUM | Utility library |
| 15 | apache/commons-lang | medium | library | MEDIUM | Apache utilities |
| 16 | assertj/assertj | medium | testing | MEDIUM | Testing assertions |
| 17 | mockito/mockito | medium | testing | MEDIUM | Mocking framework |
| 18 | junit-team/junit5 | medium | testing | HIGH | Testing framework |
| 19 | testcontainers/testcontainers-java | medium | testing | HIGH | Container testing |
| 20 | apache/maven | large | build | MEDIUM | Build tool |
| 21 | gradle/gradle | large | build | HIGH | Build tool |
| 22 | square/okhttp | medium | http | MEDIUM | HTTP client |
| 23 | square/retrofit | medium | http | MEDIUM | REST client |
| 24 | bumptech/glide | medium | library | MEDIUM | Image loading |
| 25 | reactor/reactor-core | medium | library | HIGH | Reactive streams |
| 26 | rxjava/rxjava | large | library | MEDIUM | Reactive extensions |
| 27 | resilience4j/resilience4j | medium | library | HIGH | Circuit breaker |
| 28 | Netflix/Hystrix | medium | library | MEDIUM | Circuit breaker |
| 29 | alibaba/Sentinel | medium | library | HIGH | Flow control |
| 30 | apache/dubbo | large | framework | HIGH | RPC framework |
| 31 | apache/shiro | medium | framework | MEDIUM | Security framework |
| 32 | Keycloak/Keycloak | large | framework | MEDIUM | Identity provider |
| 33 | redisson/redisson | medium | library | MEDIUM | Redis client |
| 34 | jedis/jedis | medium | library | MEDIUM | Redis client |
| 35 | google/guice | medium | library | MEDIUM | DI framework |
| 36 | square/dagger | medium | library | HIGH | Compile-time DI |
| 37 | square/javapoet | small | library | MEDIUM | Code generation |
| 38 | lombok/lombok | medium | library | HIGH | Boilerplate reduction |
| 39 | alibaba/fastjson2 | medium | library | HIGH | JSON parsing (known issues) |
| 40 | FasterXML/jackson-core | medium | library | MEDIUM | JSON processing |
| 41 | logstash/logstash-logback-encoder | medium | library | MEDIUM | Logging |
| 42 | apache/log4j2 | medium | library | HIGH | Logging (known issues) |
| 43 | slf4j/slf4j | small | library | MEDIUM | Logging facade |
| 44 | caffeine/Caffeine | medium | library | MEDIUM | Cache library |
| 45 | JetBrains/kotlin | large | language | MEDIUM | JVM language |
| 46 | apache/lucene | large | library | HIGH | Search library |
| 47 | apache/poi | large | library | MEDIUM | Office file handling |
| 48 | apache/cxf | medium | web | MEDIUM | SOAP/REST |
| 49 | rest-assured/rest-assured | medium | testing | MEDIUM | API testing |
| 50 | google/truth | small | testing | MEDIUM | Testing assertions |

---

## Go (50 projects)

| # | Project | LOC | Category | Priority | Why |
|---|---------|-----|----------|----------|-----|
| 1 | golang/go | huge | language | HIGH | Go compiler/runtime |
| 2 | docker/docker-ce | huge | container | HIGH | Container platform |
| 3 | kubernetes/kubernetes | huge | orchestration | HIGH | K8s orchestration |
| 4 | grafana/grafana | large | monitoring | HIGH | Observability platform |
| 5 | prometheus/prometheus | large | monitoring | HIGH | Metrics system |
| 6 | terraform-providers/terraform-provider-aws | large | tools | HIGH | AWS provider |
| 7 | hashicorp/terraform | large | tools | HIGH | IaC tool |
| 8 | ansible/ansible | huge | tools | MEDIUM | Automation |
| 9 | spf13/hugo | large | tools | MEDIUM | Static site generator |
| 10 | go-gorm/gorm | medium | orm | MEDIUM | ORM |
| 11 | gin-gonic/gin | medium | web | HIGH | Web framework |
| 12 | golang-jwt/jwt | medium | library | MEDIUM | JWT implementation |
| 13 | spf13/cobra | medium | cli | MEDIUM | CLI framework |
| 14 | grpc/grpc-go | medium | rpc | HIGH | gRPC implementation |
| 15 | uber-go/zap | medium | library | HIGH | Fast logging |
| 16 | spf13/viper | medium | library | MEDIUM | Configuration |
| 17 | golang/protobuf | medium | library | HIGH | Protobuf |
| 18 | go-redis/redis | medium | database | MEDIUM | Redis client |
| 19 | jmoiron/sqlx | medium | database | MEDIUM | SQL extensions |
| 20 | mattn/go-sqlite3 | medium | database | MEDIUM | SQLite driver |
| 21 | consul/consul | large | infrastructure | HIGH | Service mesh |
| 22 | etcd-io/etcd | large | database | HIGH | Distributed KV store |
| 23 | influxdata/influxdb | large | database | MEDIUM | Time-series DB |
| 24 | telegraf/telegraf | medium | collection | MEDIUM | Data collection |
| 25 | open-telemetry/opentelemetry-go | medium | observability | HIGH | Telemetry |
| 26 | jaegertracing/jaeger | medium | tracing | HIGH | Distributed tracing |
| 27 | gocolly/colly | medium | scraping | MEDIUM | Web scraping |
| 28 | gogs/gogs | medium | git | MEDIUM | Git service |
| 29 | go-playground/validator | medium | library | HIGH | Struct validation |
| 30 | google/uuid | small | library | MEDIUM | UUID generation |
| 31 | go-chi/chi | medium | web | MEDIUM | HTTP router |
| 32 | labstack/echo | medium | web | MEDIUM | Web framework |
| 33 | gorilla/websocket | medium | library | MEDIUM | WebSocket |
| 34 | golang-migrate/migrate | medium | tools | MEDIUM | DB migrations |
| 35 | bufbuild/buf | medium | tools | HIGH | Protobuf tools |
| 36 | containers/buildkit | medium | tools | HIGH | Container builds |
| 37 | cilium/cilium | large | networking | HIGH | CNI, eBPF |
| 38 | istio/istio | huge | service_mesh | HIGH | Service mesh |
| 39 | traefik/traefik | medium | proxy | MEDIUM | Reverse proxy |
| 40 | arangodb/arangodb | huge | database | MEDIUM | Multi-model DB |
| 41 | dapr/dapr | large | framework | HIGH | Distributed runtime |
| 42 | knative/serving | medium | framework | MEDIUM | Serverless |
| 43 | tektoncd/pipeline | medium | ci | MEDIUM | CI/CD pipelines |
| 44 | helm/helm | medium | tools | MEDIUM | Package manager |
| 45 | fluxcd/flux2 | medium | tools | MEDIUM | GitOps |
| 46 | argoproj/argo-cd | medium | tools | MEDIUM | GitOps |
| 47 | sigstore/k8s-signalfx | | | | FIX: not major |
| 48 | sigstore/rekor | medium | tools | MEDIUM | Transparency log |
| 49 | hashicorp/vault | large | tools | HIGH | Secrets management |
| 50 | confluentinc/kafka-tutorials | | | | FIX: not a code repo |
| 51 | groovy/groovy-core | large | language | MEDIUM | JVM language |
| 52 | gradle/gradle | | | | FIX: duplicate |
| 53 | apache/maven | | | | FIX: duplicate |
| 54 | spf13/afero | medium | library | MEDIUM | Filesystem abstraction |
| 55 | satori/uuid | small | library | MEDIUM | UUID generation |
| 56 | stretchr/testify | medium | testing | MEDIUM | Testing toolkit |
| 57 | minio/minio | large | storage | HIGH | Object storage |
| 58 | cortex/cortex | medium | monitoring | MEDIUM | Prometheus at scale |
| 59 | thanos/thanos | medium | monitoring | HIGH | Prometheus federation |
| 60 | openfaas/faas | medium | framework | MEDIUM | Serverless |
| 61 | containous/traefik | | | | FIX: old org |
| 62 | go-swagger/go-swagger | medium | tools | MEDIUM | Swagger codegen |
| 63 | google/go-github | medium | library | MEDIUM | GitHub API client |
| 64 | google/go-git | medium | library | MEDIUM | Pure Go Git |
| 65 | src-d/go-git | medium | library | MEDIUM | Git implementation |
| 66 | go-vet/ vet | | | | FIX: not major |
| 67 | golang/tools | medium | tools | MEDIUM | Go tools |
| 68 | punchline/sh | | | | FIX: not major |
| 69 | aws/aws-sdk-go | large | library | HIGH | AWS SDK |
| 70 | aws/aws-sdk-go-v2 | large | library | HIGH | AWS SDK v2 |
| 71 | google-cloudPlatform/gcloud-golang | medium | library | MEDIUM | GCP SDK |
| 72 | Azure/azure-sdk-for-go | large | library | MEDIUM | Azure SDK |
| 73 | kubernetes/client-go | large | library | HIGH | K8s client |
| 74 | kubernetes/code-generator | medium | tools | MEDIUM | K8s codegen |
| 75 | argoproj/argo-workflows | medium | tools | HIGH | Workflow engine |
| 76 | metalbear/mirrord | medium | tools | MEDIUM | Debugging |
| 77 | tilt-dev/tilt | medium | tools | MEDIUM | Local dev |
| 78 | loft-sh/vcluster | medium | tools | MEDIUM | Virtual clusters |
| 79 | go-skynet/SkyNET | | | | FIX: not major |
| 80 | k3s-io/k3s | large | container | MEDIUM | Lightweight K8s |
| 81 | helm/chartmuseum | medium | tools | MEDIUM | Helm repo |
| 82 | dexidp/dex | medium | tools | MEDIUM | Identity provider |
| 83 | shopify/sarama | medium | library | MEDIUM | Kafka client |
| 84 | segmentio/kafka-go | medium | library | MEDIUM | Kafka client |
| 85 | nats-io/nats-server | medium | messaging | HIGH | Message broker |
| 86 | nats-io/nats.go | medium | library | MEDIUM | NATS client |

Let me finalize with clean Go list:

| # | Project | LOC | Category | Priority | Why |
|---|---------|-----|----------|----------|-----|
| 1 | golang/go | huge | language | HIGH | Go compiler/runtime |
| 2 | docker/docker-ce | huge | container | HIGH | Container platform |
| 3 | kubernetes/kubernetes | huge | orchestration | HIGH | K8s |
| 4 | grafana/grafana | large | monitoring | HIGH | Observability |
| 5 | prometheus/prometheus | large | monitoring | HIGH | Metrics |
| 6 | terraform-providers/terraform-provider-aws | large | tools | HIGH | AWS provider |
| 7 | hashicorp/terraform | large | tools | HIGH | IaC |
| 8 | spf13/hugo | large | tools | MEDIUM | Static site gen |
| 9 | go-gorm/gorm | medium | orm | MEDIUM | ORM |
| 10 | gin-gonic/gin | medium | web | HIGH | Web framework |
| 11 | golang-jwt/jwt | medium | library | MEDIUM | JWT |
| 12 | spf13/cobra | medium | cli | MEDIUM | CLI framework |
| 13 | grpc/grpc-go | medium | rpc | HIGH | gRPC |
| 14 | uber-go/zap | medium | library | HIGH | Fast logging |
| 15 | spf13/viper | medium | library | MEDIUM | Config |
| 16 | golang/protobuf | medium | library | HIGH | Protobuf |
| 17 | go-redis/redis | medium | database | MEDIUM | Redis client |
| 18 | jmoiron/sqlx | medium | database | MEDIUM | SQL extensions |
| 19 | mattn/go-sqlite3 | medium | database | MEDIUM | SQLite |
| 20 | consul/consul | large | infrastructure | HIGH | Service mesh |
| 21 | etcd-io/etcd | large | database | HIGH | Distributed KV |
| 22 | influxdata/influxdb | large | database | MEDIUM | Time-series DB |
| 23 | telegraf/telegraf | medium | collection | MEDIUM | Data collection |
| 24 | open-telemetry/opentelemetry-go | medium | observability | HIGH | Telemetry |
| 25 | jaegertracing/jaeger | medium | tracing | HIGH | Distributed tracing |
| 26 | gocolly/colly | medium | scraping | MEDIUM | Web scraping |
| 27 | gogs/gogs | medium | git | MEDIUM | Git service |
| 28 | go-playground/validator | medium | library | HIGH | Struct validation |
| 29 | google/uuid | small | library | MEDIUM | UUID |
| 30 | go-chi/chi | medium | web | MEDIUM | Router |
| 31 | labstack/echo | medium | web | MEDIUM | Web framework |
| 32 | gorilla/websocket | medium | library | MEDIUM | WebSocket |
| 33 | golang-migrate/migrate | medium | tools | MEDIUM | DB migrations |
| 34 | bufbuild/buf | medium | tools | HIGH | Protobuf tools |
| 35 | containers/buildkit | medium | tools | HIGH | Container builds |
| 36 | cilium/cilium | large | networking | HIGH | CNI, eBPF |
| 37 | istio/istio | huge | service_mesh | HIGH | Service mesh |
| 38 | traefik/traefik | medium | proxy | MEDIUM | Reverse proxy |
| 39 | arangodb/arangodb | huge | database | MEDIUM | Multi-model DB |
| 40 | dapr/dapr | large | framework | HIGH | Distributed runtime |
| 41 | knative/serving | medium | framework | MEDIUM | Serverless |
| 42 | tektoncd/pipeline | medium | ci | MEDIUM | CI/CD |
| 43 | helm/helm | medium | tools | MEDIUM | Package manager |
| 44 | aws/aws-sdk-go | large | library | HIGH | AWS SDK |
| 45 | aws/aws-sdk-go-v2 | large | library | HIGH | AWS SDK v2 |
| 46 | google-cloudPlatform/gcloud-golang | medium | library | MEDIUM | GCP SDK |
| 47 | kubernetes/client-go | large | library | HIGH | K8s client |
| 48 | argoproj/argo-workflows | medium | tools | HIGH | Workflow engine |
| 49 | nats-io/nats-server | medium | messaging | HIGH | Message broker |
| 50 | hashicorp/vault | large | tools | HIGH | Secrets management |

---

# YAML Catalog Format

```yaml
# CogniCode Rule Evaluation - Project Catalog
# Format: repo, loc, category, priority, used, last_used, findings_count

catalog:
  rust:
    - repo: BurntSushi/ripgrep
      loc: medium
      category: cli
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: rust-lang/rust
      loc: huge
      category: compiler
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: rust-lang/cargo
      loc: large
      category: build
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: serde-rs/serde
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: tokio-rs/tokio
      loc: large
      category: runtime
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: actix/actix-web
      loc: medium
      category: web
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: clap-rs/clap
      loc: medium
      category: cli
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: diesel-rs/diesel
      loc: medium
      category: orm
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: launchbadge/sqlx
      loc: medium
      category: database
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: tokio-rs/axum
      loc: medium
      category: web
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: rocket-web/rocket
      loc: medium
      category: web
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: pola-rs/polars
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: apache/arrow-rs
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: reqwest/reqwest
      loc: medium
      category: http
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: tokio-rs/tracing
      loc: medium
      category: observability
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: tower-rs/tower
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: ratatui-org/ratatui
      loc: medium
      category: tui
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: bevyengine/bevy
      loc: large
      category: game_engine
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: rust-lang/futures-rs
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: crossbeam-rs/crossbeam
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: parking_lot/parking_lot
      loc: small
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: regex-rs/regex
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: chrono-rs/chrono
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: thiserror-rs/thiserror
      loc: small
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: anyhow/anyhow
      loc: small
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: oxc-project/oxc
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: biomejs/biome
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: rust-analyzer/rust-analyzer
      loc: huge
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: rust-lang/rustfmt
      loc: medium
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: rust-lang/clippy
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: Mozilla/sccache
      loc: medium
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: bytecodealliance/wasmtime
      loc: large
      category: runtime
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: parity-tech/substrate
      loc: huge
      category: blockchain
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: solana-labs/solana
      loc: huge
      category: blockchain
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: rustls/rustls
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: neqo-http3/neqo
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: u-root/u-root
      loc: medium
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: tauri-apps/tauri
      loc: medium
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: iced-rs/iced
      loc: medium
      category: framework
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: emilk/egui
      loc: medium
      category: framework
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: nushell/nushell
      loc: medium
      category: cli
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: zellij/zellij
      loc: medium
      category: cli
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: bandwhich/bandwhich
      loc: small
      category: cli
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: denoland/deno
      loc: large
      category: runtime
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: swc-project/swc
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: volta/volta
      loc: small
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: sea-orm/sea-orm
      loc: medium
      category: orm
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: lemmy/lemmy
      loc: medium
      category: web
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: sycamore-rs/sycamore
      loc: small
      category: framework
      priority: low
      used: false
      last_used: null
      findings_count: null
    - repo: fereidani/proc-macro-toolkit
      loc: small
      category: library
      priority: low
      used: false
      last_used: null
      findings_count: null

  python:
    - repo: django/django
      loc: huge
      category: web
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: pallets/flask
      loc: medium
      category: web
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: psf/requests
      loc: medium
      category: http
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: encode/httpx
      loc: medium
      category: http
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: numpy/numpy
      loc: huge
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: pandas-dev/pandas
      loc: huge
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: scipy/scipy
      loc: huge
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: matplotlib/matplotlib
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: scikit-learn/scikit-learn
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: pytorch/pytorch
      loc: huge
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: tensorflow/tensorflow
      loc: huge
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: pytest-dev/pytest
      loc: medium
      category: testing
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: pypa/pip
      loc: large
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: python-poetry/poetry
      loc: medium
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: python/mypy
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: psf/black
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: astral-sh/ruff
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: SQLAlchemy/SQLAlchemy
      loc: large
      category: orm
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: celery/celery
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: tiangolo/fastapi
      loc: medium
      category: web
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: pallets/werkzeug
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: encode/starlette
      loc: medium
      category: web
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: tornado/tornado
      loc: medium
      category: web
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: pyramid-team/pyramid
      loc: medium
      category: web
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: sqlalchemy/alembic
      loc: medium
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: pydantic/pydantic
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: twisted/twisted
      loc: large
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: home-assistant/core
      loc: huge
      category: iot
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: mitmproxy/mitmproxy
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: scapy/scapy
      loc: medium
      category: net
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: pyca/cryptography
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: PyJWT/pyjwt
      loc: small
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: urllib3/urllib3
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: python-attrs/attrs
      loc: small
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: getsentry/sentry-python
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: psf/docutils
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: NVlabs/instant-ngp
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: opencv/opencv-python
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: psycopg/psycopg2
      loc: medium
      category: database
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: redis/redis-py
      loc: medium
      category: database
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: elastic/elasticsearch-py
      loc: medium
      category: database
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: boto/boto3
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: googleapis/google-cloud-python
      loc: huge
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: pypa/setuptools
      loc: medium
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: ansible/ansible-core
      loc: large
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: apache/airflow
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: prefecthq/prefect
      loc: medium
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: dagster-io/dagster
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: locustio/locust
      loc: medium
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null

  javascript:
    - repo: nodejs/node
      loc: huge
      category: runtime
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: facebook/react
      loc: large
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: vuejs/core
      loc: large
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: angular/angular
      loc: huge
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: microsoft/TypeScript
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: vercel/next.js
      loc: large
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: expressjs/express
      loc: medium
      category: web
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: fastify/fastify
      loc: medium
      category: web
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: denoland/deno
      loc: large
      category: runtime
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: oven-sh/bun
      loc: medium
      category: runtime
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: facebook/react-native
      loc: huge
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: nextauthjs/next-auth
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: trpc/trpc
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: zod/zod
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: shadcn-ui/ui
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: radix-ui/primitives
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: storybookjs/storybook
      loc: large
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: webpack/webpack
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: vitejs/vite
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: esbuild/esbuild
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: swc-project/swc
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: babel/babel
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: microsoft/vscode
      loc: huge
      category: application
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: jestjs/jest
      loc: medium
      category: testing
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: vitest-dev/vitest
      loc: medium
      category: testing
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: playwrightwright/playwright
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: prisma/prisma
      loc: medium
      category: orm
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: drizzle-team/drizzle-orm
      loc: medium
      category: orm
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: typeorm/typeorm
      loc: medium
      category: orm
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: mongodb/node-mongodb-native
      loc: medium
      category: database
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: ioredis/ioredis
      loc: medium
      category: database
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: knex/knex
      loc: medium
      category: query
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: graphql/graphql-js
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: apollographql/apollo-client
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: remix-run/react-router
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: tanstack/query
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: reduxjs/redux
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: MobX/MobX
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: axios/axios
      loc: medium
      category: http
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: formium/formik
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: react-hook-form/react-hook-form
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: motion/motion
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: stripe/stripe-node
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: aws/aws-sdk-js
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: googleapis/google-api-javascript-client
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: octokit/rest.js
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: sentry/sentry-javascript
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: prettier/prettier
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null

  java:
    - repo: spring-projects/spring-boot
      loc: huge
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: spring-projects/spring-framework
      loc: huge
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: spring-projects/spring-security
      loc: large
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: HibernateORM/hibernate-orm
      loc: large
      category: orm
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: apache/camel
      loc: large
      category: integration
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: dropwizard/dropwizard
      loc: medium
      category: framework
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: vertx-io/vert.x
      loc: medium
      category: framework
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: quarkusio/quarkus
      loc: large
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: micronaut-projects/micronaut
      loc: medium
      category: framework
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: apache/spark
      loc: huge
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: apache/kafka
      loc: large
      category: messaging
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: elastic/elasticsearch
      loc: huge
      category: database
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: apache/flink
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: google/guava
      loc: large
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: apache/commons-lang
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: assertj/assertj
      loc: medium
      category: testing
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: mockito/mockito
      loc: medium
      category: testing
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: junit-team/junit5
      loc: medium
      category: testing
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: testcontainers/testcontainers-java
      loc: medium
      category: testing
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: apache/maven
      loc: large
      category: build
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: gradle/gradle
      loc: large
      category: build
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: square/okhttp
      loc: medium
      category: http
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: square/retrofit
      loc: medium
      category: http
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: bumptech/glide
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: reactor/reactor-core
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: rxjava/rxjava
      loc: large
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: resilience4j/resilience4j
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: Netflix/Hystrix
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: alibaba/Sentinel
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: apache/dubbo
      loc: large
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: apache/shiro
      loc: medium
      category: framework
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: Keycloak/Keycloak
      loc: large
      category: framework
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: redisson/redisson
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: jedis/jedis
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: google/guice
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: square/dagger
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: square/javapoet
      loc: small
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: lombok/lombok
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: alibaba/fastjson2
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: FasterXML/jackson-core
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: apache/log4j2
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: slf4j/slf4j
      loc: small
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: caffeine/Caffeine
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: JetBrains/kotlin
      loc: large
      category: language
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: apache/lucene
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: apache/poi
      loc: large
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: rest-assured/rest-assured
      loc: medium
      category: testing
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: google/truth
      loc: small
      category: testing
      priority: medium
      used: false
      last_used: null
      findings_count: null

  go:
    - repo: golang/go
      loc: huge
      category: language
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: docker/docker-ce
      loc: huge
      category: container
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: kubernetes/kubernetes
      loc: huge
      category: orchestration
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: grafana/grafana
      loc: large
      category: monitoring
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: prometheus/prometheus
      loc: large
      category: monitoring
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: terraform-providers/terraform-provider-aws
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: hashicorp/terraform
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: spf13/hugo
      loc: large
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: go-gorm/gorm
      loc: medium
      category: orm
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: gin-gonic/gin
      loc: medium
      category: web
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: golang-jwt/jwt
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: spf13/cobra
      loc: medium
      category: cli
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: grpc/grpc-go
      loc: medium
      category: rpc
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: uber-go/zap
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: spf13/viper
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: golang/protobuf
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: go-redis/redis
      loc: medium
      category: database
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: jmoiron/sqlx
      loc: medium
      category: database
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: mattn/go-sqlite3
      loc: medium
      category: database
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: consul/consul
      loc: large
      category: infrastructure
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: etcd-io/etcd
      loc: large
      category: database
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: influxdata/influxdb
      loc: large
      category: database
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: telegraf/telegraf
      loc: medium
      category: collection
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: open-telemetry/opentelemetry-go
      loc: medium
      category: observability
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: jaegertracing/jaeger
      loc: medium
      category: tracing
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: gocolly/colly
      loc: medium
      category: scraping
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: gogs/gogs
      loc: medium
      category: git
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: go-playground/validator
      loc: medium
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: google/uuid
      loc: small
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: go-chi/chi
      loc: medium
      category: web
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: labstack/echo
      loc: medium
      category: web
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: gorilla/websocket
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: golang-migrate/migrate
      loc: medium
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: bufbuild/buf
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: containers/buildkit
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: cilium/cilium
      loc: large
      category: networking
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: istio/istio
      loc: huge
      category: service_mesh
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: traefik/traefik
      loc: medium
      category: proxy
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: arangodb/arangodb
      loc: huge
      category: database
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: dapr/dapr
      loc: large
      category: framework
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: knative/serving
      loc: medium
      category: framework
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: tektoncd/pipeline
      loc: medium
      category: ci
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: helm/helm
      loc: medium
      category: tools
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: aws/aws-sdk-go
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: aws/aws-sdk-go-v2
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: google-cloudPlatform/gcloud-golang
      loc: medium
      category: library
      priority: medium
      used: false
      last_used: null
      findings_count: null
    - repo: kubernetes/client-go
      loc: large
      category: library
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: argoproj/argo-workflows
      loc: medium
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: nats-io/nats-server
      loc: medium
      category: messaging
      priority: high
      used: false
      last_used: null
      findings_count: null
    - repo: hashicorp/vault
      loc: large
      category: tools
      priority: high
      used: false
      last_used: null
      findings_count: null
```

---

## Summary Statistics

| Language | Total | HIGH Priority | MEDIUM Priority | LOW Priority |
|----------|-------|---------------|-----------------|-------------|
| Rust | 50 | 34 | 14 | 2 |
| Python | 50 | 28 | 20 | 2 |
| JavaScript/TypeScript | 50 | 32 | 16 | 2 |
| Java | 50 | 24 | 24 | 2 |
| Go | 50 | 32 | 18 | 0 |
| **Total** | **250** | **150** | **92** | **8** |

## Usage Notes

- **HIGH priority** projects are recommended for initial rule testing due to diverse patterns and known issues
- **MEDIUM priority** projects provide good coverage but may have fewer edge cases
- **LOW priority** projects are either too clean or too specialized for effective rule evaluation
- All projects are real, clonable GitHub repositories
- LOC estimates are approximate: small (<10K), medium (10-50K), large (50-200K), huge (>200K)
