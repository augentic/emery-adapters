.PHONY: all build clean lib typegen android-lib apk install launch run setup setup-extension setup-host setup-java-home setup-ndk verify

WORKSPACE := ..

all: build

# Codegen only — full compile assurance is `make verify`.
build: typegen

setup: setup-extension setup-host
	@chmod +x gradlew 2>/dev/null || true

setup-extension:
	@specify extension run vectis -- android setup .

setup-host:
	@test -n "$$ANDROID_HOME" || (echo "ANDROID_HOME is unset" >&2; exit 1)
	@echo "sdk.dir=$$ANDROID_HOME" > local.properties
	@$(MAKE) -s setup-java-home
	@$(MAKE) -s setup-ndk

setup-java-home:
	@if grep -q '^org.gradle.java.home=' gradle.properties 2>/dev/null; then :; \
	elif command -v /usr/libexec/java_home >/dev/null 2>&1; then \
	  JAVA21=$$(/usr/libexec/java_home -v 21 2>/dev/null); \
	  test -n "$$JAVA21" || (echo "Java 21 not found via /usr/libexec/java_home -v 21" >&2; exit 1); \
	  echo "org.gradle.java.home=$$JAVA21" >> gradle.properties; \
	elif test -n "$$JAVA_HOME"; then \
	  echo "org.gradle.java.home=$$JAVA_HOME" >> gradle.properties; \
	else \
	  echo "Java 21 home not found; set JAVA_HOME or install JDK 21" >&2; exit 1; \
	fi

setup-ndk:
	@if ! grep -q '__ANDROID_NDK_VERSION__' shared/build.gradle.kts 2>/dev/null; then :; \
	else \
	  test -n "$$ANDROID_HOME" || (echo "ANDROID_HOME is unset" >&2; exit 1); \
	  NDK=$$(ls -1 "$$ANDROID_HOME/ndk" 2>/dev/null | sort -V | tail -1); \
	  test -n "$$NDK" || (echo "No NDK under $$ANDROID_HOME/ndk" >&2; exit 1); \
	  if [ "$$(uname)" = Darwin ]; then \
	    sed -i '' "s/__ANDROID_NDK_VERSION__/$$NDK/g" shared/build.gradle.kts; \
	  else \
	    sed -i "s/__ANDROID_NDK_VERSION__/$$NDK/g" shared/build.gradle.kts; \
	  fi; \
	fi

verify: setup typegen android-lib apk

typegen: lib
	@echo "Generating Kotlin types..."
	@cd $(WORKSPACE) && RUST_LOG=info cargo run \
		--package shared \
		--bin codegen \
		--features codegen,facet_typegen \
		-- \
		--language kotlin \
		--output-dir Android/generated

lib:
	@echo "Building Rust shared library..."
	@cd $(WORKSPACE) && cargo build --features uniffi

android-lib:
	@echo "Cross-compiling Rust for Android..."
	@./gradlew :shared:cargoBuild

apk: typegen
	@echo "Building debug APK..."
	@./gradlew :app:assembleDebug

install: apk
	@./gradlew :app:installDebug

launch:
	@adb shell am start -n __ANDROID_PACKAGE__/.MainActivity

run: install launch

clean:
	@./gradlew clean
	@rm -rf generated/
