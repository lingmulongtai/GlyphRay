package com.glyphray.android.security

import android.content.Context

class TrustedHostIdentityStore(context: Context) {
    private val preferences = context.applicationContext.getSharedPreferences(
        "glyphray_trusted_host_identities",
        Context.MODE_PRIVATE,
    )

    fun verifyOrTrust(hostId: String, fingerprint: String) {
        val key = "host.$hostId.identity_sha256"
        val pinned = preferences.getString(key, null)
        require(pinned == null || pinned == fingerprint) {
            "Host identity changed for $hostId. Forget the trusted host before pairing again."
        }
        if (pinned == null) {
            check(preferences.edit().putString(key, fingerprint).commit()) {
                "Could not persist the trusted host identity"
            }
        }
    }

    fun forget(hostId: String) {
        preferences.edit().remove("host.$hostId.identity_sha256").apply()
    }
}
