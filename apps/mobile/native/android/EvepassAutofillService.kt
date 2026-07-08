// EVEPass — Android AutofillService.
//
// Declared in the manifest; it is a component of the same app, so it reads the
// app's internal cache storage directly (offline). It links the core (.so + AAR
// with UniFFI Kotlin bindings). The vaultKey comes from the Keystore only after
// BiometricPrompt; it never reaches any JS layer.
//
// This file scaffolds onFillRequest / onSaveRequest. Wire it into the RN app's
// android project and declare it in AndroidManifest with the
// BIND_AUTOFILL_SERVICE permission.

package com.evepass.autofill

import android.app.assist.AssistStructure
import android.os.CancellationSignal
import android.service.autofill.*
import android.view.autofill.AutofillId
import android.widget.RemoteViews
import com.evepass.core.matchCredentials       // UniFFI-generated
import com.evepass.core.extractCredential
import com.evepass.core.sessionFromVaultKey

class EvepassAutofillService : AutofillService() {

    override fun onFillRequest(
        request: FillRequest,
        cancellationSignal: CancellationSignal,
        callback: FillCallback
    ) {
        val structure = request.fillContexts.last().structure
        val parsed = parseFields(structure) ?: run { callback.onSuccess(null); return }

        // Biometric gate → vaultKey → Session (all inside the native process).
        BiometricVault.authenticateAndGetVaultKey(this) { vaultKey ->
            if (vaultKey == null) { callback.onSuccess(null); return@authenticateAndGetVaultKey }

            val session = sessionFromVaultKey(vaultKey)                 // core (native-only)
            val items = VaultCache.decryptedMatchItems(session)         // read shared cache
            val query = parsed.webDomain ?: parsed.packageName
            val matches = matchCredentials(items, query)                // core (eTLD+1)

            val response = FillResponse.Builder()
            for (m in matches) {
                val json = VaultCache.decryptItemJson(m.id, session)
                val cred = extractCredential(json)                      // core
                val presentation = RemoteViews(packageName, android.R.layout.simple_list_item_1)
                    .apply { setTextViewText(android.R.id.text1, "${m.title} — ${cred.username}") }
                val dataset = Dataset.Builder()
                    .setValue(parsed.usernameId, autofillValueOf(cred.username), presentation)
                    .setValue(parsed.passwordId, autofillValueOf(cred.password), presentation)
                    .build()
                response.addDataset(dataset)
            }
            callback.onSuccess(response.build())
        }
    }

    // Offer to save a newly typed credential → core save_item (via the RN bridge
    // or a direct core call once the app is unlocked).
    override fun onSaveRequest(request: SaveRequest, callback: SaveCallback) {
        // Extract username/password from the save request and hand to the core.
        // (Requires the app to be unlocked / vaultKey available.)
        callback.onSuccess()
    }

    // ── field parsing (username/password + domain/package) ───────────────────
    private data class Parsed(
        val usernameId: AutofillId,
        val passwordId: AutofillId,
        val webDomain: String?,
        val packageName: String,
    )

    private fun parseFields(structure: AssistStructure): Parsed? {
        // Walk the view nodes, collect autofill hints (username/password) and the
        // webDomain when the field is inside a WebView/browser. Omitted here for
        // brevity — this is the scaffold's main TODO.
        return null
    }

    private fun autofillValueOf(s: String) = android.view.autofill.AutofillValue.forText(s)
}
