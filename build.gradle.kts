import org.gradle.jvm.tasks.Jar

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.moddev)
}

val modId = providers.gradleProperty("mod_id").get()
val modVersion = providers.gradleProperty("mod_version").get()
val minecraftVersion = libs.versions.minecraft.get()
val neoVersion = libs.versions.neoforge.get()

group = providers.gradleProperty("mod_group_id").get()
version = modVersion

base {
    archivesName = modId
}

kotlin {
    jvmToolchain(25)
}

repositories {
    mavenCentral()
    maven("https://thedarkcolour.github.io/KotlinForForge/") {
        content {
            includeGroup("thedarkcolour")
        }
    }
}

dependencies {
    implementation(libs.kotlinforforge)
}

neoForge {
    version = neoVersion
    validateAccessTransformers = true

    runs {
        register("client") {
            client()
        }
        register("server") {
            server()
            programArgument("--nogui")
        }
    }

    mods {
        register(modId) {
            sourceSet(sourceSets.main.get())
        }
    }
}

tasks.named<Jar>("jar") {
    archiveVersion = "$modVersion-neoforge-$minecraftVersion+mc"
    manifest {
        attributes(
            "Implementation-Title" to modId,
            "Implementation-Version" to modVersion,
        )
    }
}
