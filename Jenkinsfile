// CI for RedesignedVLC (S0.8.1): formatting, then clippy and the tests on Linux, then
// SonarQube analysis with its quality gate, then, on the way to `main`, a release build and a
// GitHub release.
//
// Changes reach `main` only through pull requests: feature branch → `dev` → `staging` →
// `main`. How much runs depends on where the change is going (`ciLevel`):
// - into `dev` (and `dev` itself): nothing. These pull requests merge without CI.
// - into `staging` (and `staging` itself): everything but the release build. GitHub requires
//   it to pass, and the pull request to come from `dev`.
// - into `main` (and `main` itself): everything, the release build packaged as Linux .deb and
//   .tar.gz artifacts. GitHub requires it to pass, the pull request to come from `staging` and
//   Cargo.toml's version not to be released yet. A merge into `main` publishes the packages as
//   the GitHub release `v<version>`.
//
// Runs on one always-on Raspberry Pi (arm64 Linux) that hosts Jenkins, SonarQube and the
// build agent, triggered by GitHub's push and pull request webhooks. Windows and macOS aren't
// built here: there are no agents for them, so releases hold Linux arm64 packages only.
// Discord hears about builds from a service beside Jenkins (`ci/discord_notifier`), which
// follows every stage live, so this file says nothing to it.
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
// libzstd-dev), plus a Vulkan driver (mesa-vulkan-drivers) for the screenshot tests. It should
// have mold too (Debian: mold): every cargo call links with it when it's there, and linking a
// GPUI binary takes a Pi far less time with mold than with GNU ld. Without it builds still work,
// saying the default linker is in use.
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
        // Every cargo and rustup call uses this toolchain. It overrides `rust-toolchain.toml`, so
        // keep the two the same. Pinned, new clippy lints can't fail builds unannounced and a
        // Rust release doesn't throw the build cache away.
        RUSTUP_TOOLCHAIN = '1.100.0'
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
        // The repository releases go to, and a "Secret text" credential holding a GitHub token
        // allowed to write its contents (tags and releases).
        GITHUB_REPO = 'ItayKarp/VLC-Remake-GPUI'
        RELEASE_CREDENTIALS_ID = 'github-release'
    }

    stages {
        stage('Promotion rules') {
            when {
                beforeAgent true
                allOf { changeRequest(); expression { ciLevel() != 'none' } }
            }
            agent { label "${env.LINUX_AGENT}" }
            steps {
                checkPromotion()
            }
        }

        stage('Cache') {
            when {
                beforeAgent true
                expression { ciLevel() != 'none' }
            }
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
            when {
                beforeAgent true
                expression { ciLevel() != 'none' }
            }
            agent { label "${env.LINUX_AGENT}" }
            steps {
                sh 'rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal --component rustfmt'
                sh 'cargo fmt --check'
            }
        }

        stage('Linux') {
            when {
                beforeAgent true
                expression { ciLevel() != 'none' }
            }
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
                            runCargo "clippy --all-targets ${env.FEATURES} -- -D warnings"
                        }
                    }

                    stage('Test') {
                        steps {
                            script {
                                if (env.BACKENDS == 'all') {
                                    // The same tests as `cargo test`, failing the same way, but
                                    // instrumented: this run is SonarQube's coverage report.
                                    sh 'mkdir -p target/sonar'
                                    runCargo "llvm-cov --lcov --output-path target/sonar/lcov.info ${env.FEATURES}"
                                    stash name: 'coverage', includes: 'target/sonar/lcov.info'
                                } else {
                                    runCargo "test ${env.FEATURES}"
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
                        when {
                            expression { ciLevel() == 'release' }
                        }
                        steps {
                            // The shipping profile, thin LTO and one codegen unit, since this
                            // build is what gets released: far slower to build and link on a Pi
                            // than the others, but it only runs on the way to `main`.
                            runCargo "build --release ${env.FEATURES}"
                            script {
                                if (env.BACKENDS == 'all') {
                                    packageLinux()
                                }
                            }
                        }
                    }
                }
            }
        }

        stage('SonarQube') {
            when {
                beforeAgent true
                expression { ciLevel() != 'none' }
            }
            agent { label "${env.LINUX_AGENT}" }
            steps {
                sh 'rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal --component clippy'
                sh 'mkdir -p target/sonar'
                // Coverage from the `all` cell's test run.
                unstash 'coverage'
                // Clippy findings as JSON for SonarQube. The Linux stage already failed the
                // build on any warning; this run only reports them. The `all` cell's features,
                // so cargo replays that cell's cached results instead of checking again.
                runCargo "clippy --all-targets ${cargoFeatures('all')} --message-format=json > target/sonar/clippy.json"
                script {
                    def scanner = tool env.SONAR_SCANNER_TOOL
                    withSonarQubeEnv(env.SONARQUBE_SERVER) {
                        sh "\"${scanner}/bin/sonar-scanner\" -Dsonar.projectVersion=\"${crateVersion()}\" ${sonarProject()}"
                    }
                }
            }
        }

        stage('Quality gate') {
            // No agent: waiting holds no executor. SonarQube reports back through its
            // Jenkins webhook (Administration > Configuration > Webhooks, pointing at
            // <jenkins>/sonarqube-webhook/).
            when {
                expression { ciLevel() != 'none' }
            }
            steps {
                timeout(time: 15, unit: 'MINUTES') {
                    waitForQualityGate abortPipeline: true
                }
            }
        }

        stage('Release') {
            when {
                beforeAgent true
                branch 'main'
            }
            agent { label "${env.LINUX_AGENT}" }
            steps {
                unstash 'package'
                withCredentials([string(credentialsId: env.RELEASE_CREDENTIALS_ID, variable: 'GH_TOKEN')]) {
                    sh "bash ci/release.sh 'v${releaseVersion()}'"
                }
            }
        }
    }
}

/**
 * Runs `cargo <args>`, linking with mold when the agent has it. `mold -run` puts mold in place of
 * the linker for everything cargo starts, cargo-llvm-cov's builds included, without changing any
 * flag, so builds with and without it share the cache.
 */
void runCargo(String args) {
    sh """
        if command -v mold >/dev/null; then
            mold -run cargo ${args}
        else
            echo 'mold is not installed: linking with the default linker, which is much slower'
            cargo ${args}
        fi
    """
}

/**
 * How much this build checks, by the branch the change is going to: 'release' for `main`
 * (everything, with the release build), 'test' for `staging` (everything but the release
 * build) and 'none' for the rest, which reach `staging` before anything is enforced.
 */
String ciLevel() {
    def branch = env.CHANGE_TARGET ?: env.BRANCH_NAME
    return [main: 'release', staging: 'test'][branch] ?: 'none'
}

/**
 * Fails a pull request that skips a step of dev → staging → main, or that would release a
 * version `main` already released.
 */
void checkPromotion() {
    def source = [staging: 'dev', main: 'staging'][env.CHANGE_TARGET]
    if (source && env.CHANGE_BRANCH != source) {
        error "Pull requests into ${env.CHANGE_TARGET} come from ${source}, not ${env.CHANGE_BRANCH}"
    }
    if (env.CHANGE_TARGET == 'main') {
        def tag = "v${crateVersion()}"
        def released = sh(
            returnStatus: true,
            script: "git ls-remote --exit-code --tags 'https://github.com/${env.GITHUB_REPO}.git' 'refs/tags/${tag}' > /dev/null"
        ) == 0
        if (released) {
            error "${tag} is already released: bump the version in Cargo.toml"
        }
        echo "Merging releases ${tag}"
    }
}

/** The packages' version: the crate's, marked as a pre-release everywhere but `main`. */
String releaseVersion() {
    def version = crateVersion()
    if (env.BRANCH_NAME == 'main') {
        return version
    }
    def label = env.BRANCH_NAME.toLowerCase().replaceAll(/[^a-z0-9]/, '')
    return "${version}-${label}.${env.BUILD_NUMBER}"
}

/** Packages the `all` cell's release build, keeps it with the build, and hands it to Release. */
void packageLinux() {
    sh "bash ci/package-linux.sh \"\${CARGO_TARGET_DIR:-target}/release/RedesignedVLC\" '${releaseVersion()}'"
    archiveArtifacts artifacts: 'dist/*', fingerprint: true
    stash name: 'package', includes: 'dist/*'
}

/**
 * The SonarQube project this build reports to: one per long-lived branch, since the Community
 * Build analyzes one branch per project. A pull request reports to its target's project, as
 * the merge it builds is what that branch would become.
 */
String sonarProject() {
    def branch = env.CHANGE_TARGET ?: env.BRANCH_NAME
    def key = branch == 'main' ? 'redesigned-vlc' : "redesigned-vlc-${branch}"
    return "-Dsonar.projectKey=${key} -Dsonar.projectName='RedesignedVLC (${branch})'"
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
