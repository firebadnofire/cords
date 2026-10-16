# Distribution inventory

This development package is not a signed production release. The application
license is included here; bundled codec notices are under avatar-codec/licenses.
Qt is dynamically linked. Release packaging must include notices and corresponding
source or a compliant source offer for the exact deployed Qt modules and plugins,
and permit replacement of the Qt libraries under the selected license terms.

The deployment manifest is install_manifest.txt in the build directory. Qt's
deployment tool can include transitive modules and plugins beyond direct links.
Audit that manifest and Qt's SDK SBOM before release. Build success does not
establish license compliance, signing, notarization or clean-machine acceptance.
