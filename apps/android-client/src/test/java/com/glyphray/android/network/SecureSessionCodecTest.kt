package com.glyphray.android.network

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class SecureSessionCodecTest {
    @Test
    fun directionalKeyDerivationMatchesRustVector() {
        val sharedSecret = ByteArray(32) { (it * 7).toByte() }
        val transcript = ByteArray(32) { (it * 11).toByte() }
        val keys = DirectionalSessionKeys.forClient(sharedSecret, transcript)

        assertEquals(
            "13a86c080847160ebf3331bdddd11ad8377be092698e6809c3af81fbf7c6dd0e",
            keys.outbound.toHex(),
        )
        assertEquals(
            "f6daad80d2a79845aa4b0f67abac4ea0412a78ff2ffcdb029874375639bc498d",
            keys.inbound.toHex(),
        )
    }

    @Test
    fun directionalCodecsRoundTripAndRejectReplay() {
        val sharedSecret = ByteArray(32) { (it * 7).toByte() }
        val transcript = ByteArray(32) { (it * 11).toByte() }
        val sessionId = ByteArray(16) { (it + 1).toByte() }
        val clientKeys = DirectionalSessionKeys.forClient(sharedSecret, transcript)
        val hostKeys = DirectionalSessionKeys.forHost(sharedSecret, transcript)
        val client = SecureDatagramCodec(clientKeys.outbound, clientKeys.inbound, sessionId)
        val host = SecureDatagramCodec(hostKeys.outbound, hostKeys.inbound, sessionId)

        val sealed = client.seal("stylus".toByteArray())
        assertArrayEquals("stylus".toByteArray(), host.open(sealed))
        assertThrows(IllegalArgumentException::class.java) { host.open(sealed) }

        val video = host.seal("video".toByteArray())
        assertArrayEquals("video".toByteArray(), client.open(video))
    }

    @Test
    fun replayWindowAllowsUnseenReorderedDatagram() {
        val keys = DirectionalSessionKeys.forClient(ByteArray(32) { 1 }, ByteArray(32) { 2 })
        val reverse = DirectionalSessionKeys.forHost(ByteArray(32) { 1 }, ByteArray(32) { 2 })
        val sender = SecureDatagramCodec(keys.outbound, keys.inbound, ByteArray(16) { 3 })
        val receiver = SecureDatagramCodec(reverse.outbound, reverse.inbound, ByteArray(16) { 3 })
        val first = sender.seal(byteArrayOf(1))
        val second = sender.seal(byteArrayOf(2))

        assertArrayEquals(byteArrayOf(2), receiver.open(second))
        assertArrayEquals(byteArrayOf(1), receiver.open(first))
    }
}

private fun ByteArray.toHex(): String = joinToString(separator = "") { byte ->
    "%02x".format(byte)
}
