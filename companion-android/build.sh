#!/bin/sh
# Builds NetSketch-Survey.apk with the Android SDK's own tools (no Gradle needed).
# Needs: JDK 17+, Android SDK with build-tools and platform 35 (ANDROID_HOME, default ~/Android/Sdk).
set -eu
cd "$(dirname "$0")"
SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
BT="$SDK/build-tools/$(ls "$SDK/build-tools" | sort -V | tail -1)"
JAR="$SDK/platforms/android-35/android.jar"
OUT=build
rm -rf "$OUT"
mkdir -p "$OUT/classes" "$OUT/dex" "$OUT/gen" "$OUT/test"

# 1. The plain-Java part, checked on this computer (also refreshes the sample file the main app tests with).
javac -d "$OUT/test" src/org/netsketch/survey/Survey.java src/org/netsketch/survey/Link.java src/org/netsketch/survey/T.java test/SurveyJsonTest.java
java -cp "$OUT/test" SurveyJsonTest sample.netsurvey

# 2. Resources and manifest.
"$BT/aapt2" compile --dir res -o "$OUT/res.zip"
"$BT/aapt2" link -I "$JAR" --manifest AndroidManifest.xml --min-sdk-version 29 --target-sdk-version 35 \
    --java "$OUT/gen" -o "$OUT/unsigned.apk" "$OUT/res.zip"

# 3. Code.
javac --release 17 -Xlint:-options -classpath "$JAR" -d "$OUT/classes" $(find src "$OUT/gen" -name '*.java')
"$BT/d8" --release --min-api 29 --lib "$JAR" --output "$OUT/dex" $(find "$OUT/classes" -name '*.class')
cp "$OUT/unsigned.apk" "$OUT/app.apk"
(cd "$OUT/dex" && zip -q ../app.apk classes.dex)

# 4. Align and sign (a local key, made once; fine for installing on your own phone).
"$BT/zipalign" -f -p 4 "$OUT/app.apk" "$OUT/aligned.apk"
[ -f signing.keystore ] || keytool -genkeypair -keystore signing.keystore -storepass netsketch -keypass netsketch \
    -alias survey -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=NetSketch Survey" >/dev/null 2>&1
"$BT/apksigner" sign --ks signing.keystore --ks-pass pass:netsketch --key-pass pass:netsketch --out NetSketch-Survey.apk "$OUT/aligned.apk"
rm -f NetSketch-Survey.apk.idsig
echo "Built $(pwd)/NetSketch-Survey.apk"
