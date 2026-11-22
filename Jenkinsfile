// CI for RedesignedVLC (S0.8.1): formatting, then clippy, the tests and a release build on
// Linux, then SonarQube analysis with its quality gate. Results go to Discord.
//
// Runs on one always-on Raspberry Pi (arm64 Linux) that hosts Jenkins, SonarQube and the
// build agent, triggered by GitHub's push webhook for `main`. Windows and macOS aren't built
// here: there are no agents for them.
//
// Before the first run, set up Jenkins as the "Jenkins setup" block in `environment` below
// says: the agent label, the SonarQube server and scanner, and the Discord webhook credential.
//
// Plugins: Pipeline, Pipeline: Declarative (1.5 or later, for `matrix`), Multibranch Pipeline
// with a GitHub or Git source, Timestamper, Credentials Binding, SonarQube Scanner and Discord
// Notifier.
//
// The agent needs rustup, Git, pkg-config, cargo-llvm-cov and the GPUI build libraries
// (Debian: libfontconfig-dev libvulkan1 libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev
// libzstd-dev), plus a Vulkan driver (mesa-vulkan-drivers) for the screenshot tests.

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
        // Every cargo and rustup call uses this toolchain. Pin a version (for example
        // '1.100.0') to keep new clippy lints from failing builds unannounced.
        RUSTUP_TOOLCHAIN = 'stable'

        // ---- Jenkins setup: fill these in when configuring Jenkins. ----
        // The label of the Linux agent. The matrix below uses the literal 'linux' (Jenkins
        // needs one there); keep the two the same.
        LINUX_AGENT = 'linux'
        // Manage Jenkins > System > SonarQube servers: the server's name.
        SONARQUBE_SERVER = 'sonarqube'
        // Manage Jenkins > Tools > SonarQube Scanner installations: the scanner's name.
        SONAR_SCANNER_TOOL = 'sonar-scanner'
        // Credentials: a "Secret text" credential holding the Discord webhook URL.
        DISCORD_WEBHOOK_CREDENTIALS_ID = 'discord-webhook-url'
        // 'true' also posts passing builds; failures, unstable builds and fixes always post.
        DISCORD_NOTIFY_ON_SUCCESS = 'false'
    }

    stages {
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
                            sh "rustup toolchain install ${env.RUSTUP_TOOLCHAIN} --profile minimal --component clippy"
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
                            sh "cargo test ${env.FEATURES}"
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
                sh 'rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal --component clippy,llvm-tools-preview'
                sh 'mkdir -p target/sonar'
                // Clippy findings as JSON for SonarQube. The Linux stage already failed the
                // build on any warning; this run only reports them.
                sh 'cargo clippy --all-targets --message-format=json > target/sonar/clippy.json'
                sh 'cargo llvm-cov --lcov --output-path target/sonar/lcov.info'
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

    post {
        always {
            notifyDiscord()
        }
    }
}

/** The cargo flags that build `backends`: 'all' is the default features. */
String cargoFeatures(String backends) {
    return backends == 'all' ? '' : "--no-default-features --features ${backends}"
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
    sh "rm -rf '${folder}' && mkdir -p '${folder}' && { cp target/screenshots/*.png '${folder}/' 2>/dev/null || true; }"
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

/** Posts the build's result to Discord: always for failures, unstable builds and fixes. */
void notifyDiscord() {
    def result = currentBuild.currentResult
    def previous = currentBuild.previousBuild?.result
    def fixed = result == 'SUCCESS' && previous != null && previous != 'SUCCESS'
    if (result == 'SUCCESS' && !fixed && env.DISCORD_NOTIFY_ON_SUCCESS != 'true') {
        return
    }
    def status = fixed ? 'fixed' : result.toLowerCase()
    def branch = env.CHANGE_ID ? "PR #${env.CHANGE_ID} (${env.CHANGE_BRANCH})" : (env.BRANCH_NAME ?: 'unknown')
    withCredentials([string(credentialsId: env.DISCORD_WEBHOOK_CREDENTIALS_ID, variable: 'DISCORD_WEBHOOK_URL')]) {
        discordSend(
            webhookURL: env.DISCORD_WEBHOOK_URL,
            title: "${env.JOB_NAME} #${env.BUILD_NUMBER}: ${status}",
            link: env.BUILD_URL,
            result: result,
            description: "Branch: ${branch}\nDuration: ${currentBuild.durationString.replace(' and counting', '')}",
            footer: 'RedesignedVLC CI'
        )
    }
}
