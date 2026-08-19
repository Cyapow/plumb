plugins {
    id("java")
    id("org.jetbrains.kotlin.jvm") version "1.9.24"
    id("org.jetbrains.intellij.platform") version "2.0.1"
}

group = "com.plumb"
version = "0.1.2"

repositories {
    mavenCentral()
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    intellijPlatform {
        // Community edition is enough — we only use the platform + JCEF.
        intellijIdeaCommunity("2024.1")
        instrumentationTools()
    }
}

intellijPlatform {
    pluginConfiguration {
        ideaVersion {
            // Broad compatibility: the plugin only uses stable platform APIs
            // (tool window + JCEF), so don't let the build cap the upper bound to
            // the compile-time platform (which rejects newer IDEs like 2026.x).
            sinceBuild = "233"
            untilBuild = "299.*"
        }
    }
}

kotlin {
    jvmToolchain(17)
}
