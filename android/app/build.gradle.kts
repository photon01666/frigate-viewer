plugins {
    id("com.android.application")
}

android {
    namespace = "com.frigateviewer.app"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.frigateviewer.app"
        minSdk = 26
        targetSdk = 34
        versionCode = 2
        versionName = "1.0.0"

        ndk {
            // The Rust library is built for this ABI by build.sh.
            abiFilters += listOf("arm64-v8a")
        }
    }

    buildFeatures {
        buildConfig = true
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            isShrinkResources = false
            // Personal sideload build. Replace with your own signing config for
            // a Play Store release.
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }
}
