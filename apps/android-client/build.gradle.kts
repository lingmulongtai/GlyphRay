plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val glyphRayVersionName = providers.environmentVariable("GLYPHRAY_VERSION_NAME")
    .orElse(rootProject.file("VERSION").readText().trim())
    .get()
val glyphRayVersionCode = providers.environmentVariable("GLYPHRAY_VERSION_CODE")
    .orElse("1")
    .get()
    .toInt()
val releaseStorePath = providers.environmentVariable("GLYPHRAY_ANDROID_KEYSTORE").orNull
val releaseStorePassword = providers.environmentVariable("GLYPHRAY_ANDROID_STORE_PASSWORD").orNull
val releaseKeyAlias = providers.environmentVariable("GLYPHRAY_ANDROID_KEY_ALIAS").orNull
val releaseKeyPassword = providers.environmentVariable("GLYPHRAY_ANDROID_KEY_PASSWORD").orNull
val hasReleaseSigning = listOf(
    releaseStorePath,
    releaseStorePassword,
    releaseKeyAlias,
    releaseKeyPassword,
).all { !it.isNullOrBlank() }

android {
    namespace = "com.glyphray.android"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.glyphray.android"
        minSdk = 26
        targetSdk = 35
        versionCode = glyphRayVersionCode
        versionName = glyphRayVersionName
    }

    signingConfigs {
        if (hasReleaseSigning) {
            create("release") {
                storeFile = file(requireNotNull(releaseStorePath))
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        }
    }

    buildTypes {
        getByName("release") {
            isMinifyEnabled = false
            if (hasReleaseSigning) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }
}

configurations.configureEach {
    if (name.endsWith("RuntimeClasspathCopy")) {
        isCanBeConsumed = false
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2024.10.00"))
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.foundation:foundation-layout")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.runtime:runtime")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    debugImplementation("androidx.compose.ui:ui-tooling")
    testImplementation("junit:junit:4.13.2")
}
