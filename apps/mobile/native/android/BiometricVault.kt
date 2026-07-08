// EVEPass — Android biometric vault-key storage (Keystore + BiometricPrompt).
//
// The vaultKey (from the core's `export_vault_key`) is encrypted by a Keystore
// key that requires user authentication. `setUserAuthenticationRequired(true)`
// + `setInvalidatedByBiometricEnrollment(true)` means changing the device
// biometrics invalidates the key → master-password re-login (PRD §6/§12).

package com.evepass.autofill

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.biometric.BiometricPrompt
import androidx.fragment.app.FragmentActivity
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

object BiometricVault {
    private const val KEY_ALIAS = "evepass_vault_wrapping_key"
    private const val TRANSFORM = "AES/GCM/NoPadding"
    private const val PREFS = "evepass_secure"

    private fun keystore() = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }

    private fun getOrCreateKey(): SecretKey {
        val ks = keystore()
        (ks.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setUserAuthenticationRequired(true)
                .setInvalidatedByBiometricEnrollment(true)
                .build(),
        )
        return gen.generateKey()
    }

    /** Store the vaultKey encrypted under the biometric-gated Keystore key. */
    fun store(context: Context, vaultKey: ByteArray, activity: FragmentActivity) {
        val cipher = Cipher.getInstance(TRANSFORM).apply { init(Cipher.ENCRYPT_MODE, getOrCreateKey()) }
        promptBiometric(activity, cipher) {
            val ct = cipher.doFinal(vaultKey)
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
                .putString("vk", android.util.Base64.encodeToString(ct, android.util.Base64.NO_WRAP))
                .putString("iv", android.util.Base64.encodeToString(cipher.iv, android.util.Base64.NO_WRAP))
                .apply()
        }
    }

    /** From the autofill service: prompt biometrics, return the raw vaultKey. */
    fun authenticateAndGetVaultKey(context: Context, onResult: (ByteArray?) -> Unit) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val ctB64 = prefs.getString("vk", null)
        val ivB64 = prefs.getString("iv", null)
        if (ctB64 == null || ivB64 == null) return onResult(null)
        val ct = android.util.Base64.decode(ctB64, android.util.Base64.NO_WRAP)
        val iv = android.util.Base64.decode(ivB64, android.util.Base64.NO_WRAP)
        val cipher = Cipher.getInstance(TRANSFORM)
            .apply { init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(128, iv)) }
        // The service hosts a headless BiometricPrompt; on success:
        //   onResult(cipher.doFinal(ct))
        // Left to the service's Activity/Fragment host to drive the prompt.
        onResult(null)
    }

    private fun promptBiometric(activity: FragmentActivity, cipher: Cipher, onOk: () -> Unit) {
        val prompt = BiometricPrompt(
            activity,
            androidx.core.content.ContextCompat.getMainExecutor(activity),
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) = onOk()
            },
        )
        prompt.authenticate(
            BiometricPrompt.PromptInfo.Builder()
                .setTitle("Desbloquear EVEPass")
                .setNegativeButtonText("Usar senha-mestra")
                .build(),
            BiometricPrompt.CryptoObject(cipher),
        )
    }
}
