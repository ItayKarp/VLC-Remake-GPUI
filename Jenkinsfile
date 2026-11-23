// CI for RedesignedVLC (S0.8.1): formatting, then clippy, the tests and a release build on
// Linux, then SonarQube analysis with its quality gate.
//
// Runs on one always-on Raspberry Pi (arm64 Linux) that hosts Jenkins, SonarQube and the
// build agent, triggered by GitHub's push webhook for `main`. Windows and macOS aren't built
// here: there are no agents for them. Discord hears about builds from a service beside Jenkins
// (`ci/discord_notifier`), which follows every stage live, so this file says nothing to it.
//
// Before the first run, set up Jenkins as the "Jenkins setup" block in `environment` below
// says: the agent label, and the SonarQube server and scanner.
//
// Plugins: Pipeline, Pipeline: Declarative (1.5 or later, for `matrix`), Pipeline Graph View
// (the notifier reads its stage tree), Multibranch Pipeline with a GitHub or Git source,
// Timestamper and SonarQube Scanner.
//
// The agent needs rustup, Git, pkg-config, cargo-llvm-cov and the GPUI build libraries
// (Debian: libfontconfig-dev libvulkan1 libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev
// libzstd-dev), plus a Vulkan driver (mesa-vulkan-drivers) for the screenshot tests.
//
// Caching: builds reuse cargo's compiled crates instead of starting over, and check exactly as
// much. Set CARGO_TARGET_DIR in the agent's environment to a folder outside the workspace so
// the cache outlives the job's workspace; without it the cache is the workspace's `target`.
// Cargo rebuilds whatever a change touches, and replays cached warnings, so a cached build
// fails on everything an uncached one does. The Cache stage clears the cache when it outgrows
// CARGO_CACHE_LIMIT_GB or the disk runs low, so it never fills the disk. The `all` cell's tests
// run under cargo-llvm-cov, giving SonarQube its coverage without a second build and test run.

pipeline {
    agent none

    options {
        timestamps()
        // A Pi builds everything one cell at a time; an uncached build takes hours.
        timeout(time: 6, unit: 'HOURS')
        buildDiscarder(logRotator(numToKeepStr: '30', artifactNumToKeepStr: '10'))
        disableConcurrentBuilds(abortPrevious: true)
    }

    environment {
        CARGO_TERM_COLOR = 'always'
        RUST_BACKTRACE = '1'
        // Debug and test builds keep file and line numbers (for panics and backtraces) but not
        // the full debug info a debugger reads. A GPUI binary with full debug info outgrows the
        // Pi's memory while linking; this checks and tests exactly the same code.
        CARGO_PROFILE_DEV_DEBUG = 'line-tables-only'
        // Every cargo and rustup call uses this toolchain. Pin a version (for example
        // '1.100.0') to keep new clippy lints from failing builds unannounced.
        RUSTUP_TOOLCHAIN = 'stable'
        // The build cache's size limit, and the free disk space below which it is cleared too.
        CARGO_CACHE_LIMIT_GB = '60'
        CARGO_CACHE_MIN_FREE_GB = '15'

        // ---- Jenkins setup: fill these in when configuring Jenkins. ----
        // The label of the Linux agent. The matrix below uses the literal 'linux' (Jenkins
        // needs one there); keep the two the same.
        LINUX_AGENT = 'linux'
        // Manage Jenkins > System > SonarQube servers: the server's name.
        SONARQUBE_SERVER = 'sonarqube'
        // Manage Jenkins > Tools > SonarQube Scanner installations: the scanner's name.
        SONAR_SCANNER_TOOL = 'sonar-scanner'
    }

    stages {
        stage('Cache') {
            agent { label "${env.LINUX_AGENT}" }
            steps {
                // Single quotes: the shell, not Groovy, expands these variables.
                sh '''
                    cache="${CARGO_TARGET_DIR:-target}"
                    mkdir -p "$cache"
                    used=$(du -s --block-size=1G "$cache" | cut -f1)
                    free=$(df --output=avail --block-size=1G "$cache" | tail -n 1 | tr -dc 0-9)
                    echo "Build cache $cache: ${used} GB used (limit ${CARGO_CACHE_LIMIT_GB}), ${free} GB free on disk"
                    if [ "$used" -ge "$CARGO_CACHE_LIMIT_GB" ] || [ "$free" -lt "$CARGO_CACHE_MIN_FREE_GB" ]; then
                        echo "Clearing the build cache: this build compiles everything again"
                        rm -rf "$cache"
                    fi
                '''
            }
        }

        stage('Format') {
            agent { label "${env.LINUX_AGENT}" }
            steps {
                sh 'rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal --component rustfmt'
                sh 'cargo fmt --check'
            }
        }

        stage('Linux') {
            matrix {
                axes {
                    // Display servers: both, or each one alone (the `x11` and `wayland`
                    // crate features).
                    axis {
                        name 'BACKENDS'
                        values 'all', 'x11', 'wayland'
                    }
                }

                agent { label 'linux' }

                environment {
                    FEATURES = cargoFeatures("${BACKENDS}")
                }

                stages {
                    stage('Toolchain') {
                        steps {
                            sh "rustup toolchain install ${env.RUSTUP_TOOLCHAIN} --profile minimal --component clippy,llvm-tools-preview"
                        }
                    }

                    stage('Libraries') {
                        steps {
                            // Fails early, naming what's missing, instead of deep in a build script.
                            sh "pkg-config --print-errors --exists ${linuxLibraries(env.BACKENDS)}"
                        }
                    }

                    stage('Clippy') {
                        steps {
                            sh "cargo clippy --all-targets ${env.FEATURES} -- -D warnings"
                        }
                    }

                    stage('Test') {
                        steps {
                            script {
                                if (env.BACKENDS == 'all') {
                                    // The same tests as `cargo test`, failing the same way, but
                                    // instrumented: this run is SonarQube's coverage report.
                                    sh "mkdir -p target/sonar && cargo llvm-cov --lcov --output-path target/sonar/lcov.info ${env.FEATURES}"
                                    stash name: 'coverage', includes: 'target/sonar/lcov.info'
                                } else {
                                    sh "cargo test ${env.FEATURES}"
                                }
                            }
                        }
                        post {
                            always {
                                archiveScreenshots()
                            }
                        }
                    }

                    stage('Release build') {
                        steps {
                            sh "cargo build --release ${env.FEATURES}"
                        }
                    }
                }
            }
        }

        stage('SonarQube') {
            agent { label "${env.LINUX_AGENT}" }
            steps {
                sh 'rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal --component clippy'
                sh 'mkdir -p target/sonar'
                // Coverage from the `all` cell's test run.
                unstash 'coverage'
                // Clippy findings as JSON for SonarQube. The Linux stage already failed the
                // build on any warning; this run only reports them. The `all` cell's features,
                // so cargo replays that cell's cached results instead of checking again.
                sh "cargo clippy --all-targets ${cargoFeatures('all')} --message-format=json > target/sonar/clippy.json"
                script {
                    def scanner = tool env.SONAR_SCANNER_TOOL
                    withSonarQubeEnv(env.SONARQUBE_SERVER) {
                        sh "\"${scanner}/bin/sonar-scanner\" -Dsonar.projectVersion=\"${crateVersion()}\""
                    }
                }
            }
        }

        stage('Quality gate') {
            // No agent: waiting holds no executor. SonarQube reports back through its
            // Jenkins webhook (Administration > Configuration > Webhooks, pointing at
            // <jenkins>/sonarqube-webhook/).
            steps {
                timeout(time: 15, unit: 'MINUTES') {
                    waitForQualityGate abortPipeline: true
                }
            }
        }
    }
}

/**
 * The cargo flags that build `backends`: 'all' is both, the same as the default features.
 * Never empty: Jenkins makes an empty `environment` value null, which cargo would get as "null".
 */
String cargoFeatures(String backends) {
    def features = backends == 'all' ? 'x11,wayland' : backends
    return "--no-default-features --features ${features}"
}

/** The pkg-config names of the system libraries a Linux build of `backends` links. */
String linuxLibraries(String backends) {
    def libraries = ['fontconfig', 'xkbcommon']
    if (backends != 'wayland') {
        libraries += ['x11-xcb', 'xkbcommon-x11']
    }
    if (backends != 'x11') {
        libraries += ['wayland-client']
    }
    return libraries.join(' ')
}

/**
 * Keeps the screenshot tests' actual, expected and diff images, under the cell's name so the
 * cells don't overwrite each other's.
 */
void archiveScreenshots() {
    def folder = "screenshots/linux-${env.BACKENDS}"
    // The tests write into cargo's target folder, which CARGO_TARGET_DIR may move, and under
    // cargo-llvm-cov (the `all` cell) into its own folder inside it.
    sh """
        rm -rf '${folder}' && mkdir -p '${folder}'
        target="\${CARGO_TARGET_DIR:-target}"
        for images in "\$target/screenshots" "\$target/llvm-cov-target/screenshots"; do
            cp "\$images"/*.png '${folder}/' 2>/dev/null || true
        done
    """
    archiveArtifacts artifacts: "${folder}/*.png", allowEmptyArchive: true
}

/** The `version` in Cargo.toml's [package] table. */
String crateVersion() {
    return packageVersion(readFile('Cargo.toml'))
}

/** The first `version = "…"` line of `manifest`, which is the [package] table's. */
@NonCPS
String packageVersion(String manifest) {
    // NonCPS: a regex Matcher can't be saved between pipeline steps.
    def match = manifest =~ /(?m)^version\s*=\s*"([^"]+)"/
    return match.find() ? match.group(1) : 'unknown'
}
