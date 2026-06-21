---
name: encryption-and-hashing
description: Use when implementing encryption, decryption, hashing, signing, or any cryptographic operation. Load when a task involves storing sensitive data, verifying integrity, generating tokens, handling certificates, or any situation where "secure" matters in a cryptographic sense.
---

# Encryption and Hashing

## Overview

Cryptography is the one area where "it seems to work" is not good enough. A homebrew encryption scheme that produces output that looks random is still trivially breakable by anyone who knows how. The rules are absolute: use established algorithms, use established libraries, and follow established patterns. This skill covers the cryptographic operations developers encounter most, with clear guidance on what to use and what to never do.

## Hashing vs. Encryption

| | Hashing | Encryption |
|---|---|---|
| **Direction** | One-way | Two-way |
| **Purpose** | Integrity verification, password storage | Confidentiality |
| **Output** | Fixed-length digest | Variable-length ciphertext |
| **Key** | No key (or secret key for HMAC) | Requires key |
| **Examples** | SHA-256, bcrypt, HMAC-SHA256 | AES-GCM, RSA-OAEP |

**Never confuse them**: Don't "encrypt" passwords (you need to verify, not decrypt). Don't "hash" data you need to read back.

## Password Hashing

### Only Use Purpose-Built Algorithms

| Algorithm | Use it? | Why |
|-----------|---------|-----|
| **bcrypt** | ✅ Yes | Battle-tested, built-in salt, configurable cost |
| **argon2id** | ✅ Best | Winner of Password Hashing Competition, resistant to GPU/ASIC attacks |
| **scrypt** | ✅ Yes | Memory-hard, good for high-security contexts |
| **PBKDF2** | ⚠️ Acceptable | Weaker against GPU attacks, but widely available |
| **SHA-256/512** | ❌ No | Too fast — brute-forceable at billions/second |
| **MD5, SHA-1** | ❌ Never | Broken, collision-prone, catastrophically fast |

### Implementation Pattern

```python
# Python with bcrypt
import bcrypt

# Hashing (on registration)
password_bytes = password.encode('utf-8')
salt = bcrypt.gensalt(rounds=12)  # Cost factor: 12 is a good default
hashed = bcrypt.hashpw(password_bytes, salt)

# Verifying (on login)
bcrypt.checkpw(password_bytes, hashed)  # Returns True/False
```

```typescript
// Node.js with argon2
import argon2 from 'argon2';

// Hashing
const hash = await argon2.hash(password, { type: argon2.argon2id });

// Verifying
const valid = await argon2.verify(hash, password);
```

### Key Points
- **Never store plain-text passwords**
- **Never use MD5 or SHA** for passwords — they're too fast
- **The salt is included in the hash output** — you don't need a separate column
- **Cost factor**: Higher = slower = harder to crack, but slower for legitimate logins. Target ~100-250ms per hash on your hardware.
- **Timing-safe comparison**: Use constant-time comparison to prevent timing attacks. `bcrypt.checkpw` and `argon2.verify` do this internally.

## Symmetric Encryption

Use **AES-GCM** for almost all symmetric encryption needs.

### Why AES-GCM

- **AES**: Standard block cipher, widely audited, hardware-accelerated
- **GCM mode**: Provides both encryption and authentication (AEAD). Detects tampering.
- **Never use ECB mode**: Identical plaintext blocks produce identical ciphertext blocks (the "ECB penguin" attack)

### Implementation Pattern

```python
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

# Key generation (do this once, store securely)
key = AESGCM.generate_key(bit_length=256)  # 32 bytes

# Encryption
aesgcm = AESGCM(key)
nonce = os.urandom(12)  # 96-bit nonce, MUST be unique per key
ciphertext = aesgcm.encrypt(nonce, plaintext, associated_data)

# Decryption
plaintext = aesgcm.decrypt(nonce, ciphertext, associated_data)
```

### Critical Rules

- **Nonce must never repeat with the same key**. Generate a random 96-bit nonce for each encryption. The probability of collision is negligible.
- **Key must be 256 bits** (32 bytes). 128-bit keys are acceptable but 256 is the standard.
- **Associated data (AAD)**: Data that's authenticated but not encrypted (e.g., a user ID). Prevents ciphertext from being moved between contexts.
- **Never reuse nonce + key combination**. This completely breaks GCM.

## Asymmetric Encryption

### Use Cases

| Use case | Algorithm | Why |
|----------|-----------|-----|
| **Key exchange** | ECDH (P-256, X25519) | Establish a shared secret over an insecure channel |
| **Digital signatures** | ECDSA (P-256), Ed25519 | Prove a message came from a specific key holder |
| **Encrypting small data** | RSA-OAEP | Encrypt with public key, decrypt with private key |
| **Encrypting large data** | Hybrid: RSA + AES | RSA to encrypt a symmetric key, AES for the data |

### Key Sizes

| Algorithm | Minimum | Recommended |
|-----------|---------|-------------|
| RSA | 2048 bits | 4096 bits |
| ECDSA (P-256) | 256 bits | 256 bits (equivalent to 3072-bit RSA) |
| Ed25519 | 256 bits | 256 bits (preferred for new systems) |
| ECDH (X25519) | 256 bits | 256 bits (preferred for new systems) |

**Prefer elliptic curve** (EC) algorithms over RSA for new systems: smaller keys, faster operations, same security level.

## Signing

### HMAC (Symmetric)

For message authentication when both parties share a secret:

```python
import hmac
import hashlib

key = b"shared-secret-key"
message = b"important data"
signature = hmac.new(key, message, hashlib.sha256).hexdigest()

# Verification
hmac.new(key, message, hashlib.sha256).hexdigest() == signature
```

Use HMAC when: API request signing, webhook verification, JWT signing with a shared secret.

### Digital Signatures (Asymmetric)

For non-repudiation — the signer can't deny having signed:

```python
from cryptography.hazmat.primitives.asymmetric import ed25519

# Key generation
private_key = ed25519.Ed25519PrivateKey.generate()
public_key = private_key.public_key()

# Signing
signature = private_key.sign(message)

# Verification
public_key.verify(signature, message)  # Raises InvalidSignature if invalid
```

Use digital signatures when: software releases, certificate authorities, any situation where the verifier doesn't have the signing key.

## Key Management

The hardest part of cryptography isn't the algorithms — it's the keys.

### Key Generation

- **Always use cryptographically secure random**: `os.urandom()`, `crypto.randomBytes()`, `crypto/rand.Read`
- **Never use `Math.random()`**, `random.random()`, or any non-crypto PRNG for keys, tokens, or nonces

### Key Storage

| Storage | Security | When to use |
|---------|----------|-------------|
| **HSM / KMS** | Highest | Production secrets, payment processing |
| **Vault / Secret manager** | High | Application secrets, API keys |
| **Environment variables** | Medium | Deployment secrets (better than files) |
| **Config files** | Low | Dev only, never in git |
| **Hardcoded** | None | **Never** |

### Key Rotation

- Rotate keys periodically (every 90 days for high-security, annually for standard)
- Support multiple active keys during rotation (old key decrypts, new key encrypts)
- Never delete old keys immediately — existing data encrypted with them becomes unreadable

### Key Derivation

When you need a key from a password or shared secret:

```python
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from cryptography.hazmat.primitives import hashes

# Derive a 32-byte key from a shared secret
key = HKDF(
    algorithm=hashes.SHA256(),
    length=32,
    salt=salt_bytes,       # Random, unique per derivation
    info=b"encryption-key",  # Context-specific label
).derive(shared_secret)
```

**Use HKDF** for deriving keys from shared secrets. **Use PBKDF2/bcrypt/argon2** for deriving keys from passwords (they're slow, which is the point).

## Common Mistakes

| Mistake | Why it's bad | Fix |
|---------|-------------|-----|
| ECB mode | Identical blocks → identical ciphertext | Use GCM or CBC with HMAC |
| Rolling your own crypto | You will make subtle mistakes | Use established libraries |
| Reusing nonce | Breaks GCM confidentiality | Generate random nonce per encryption |
| Weak key size | Brute-forceable | RSA ≥2048, AES-256 |
| `Math.random()` for secrets | Predictable | Use `crypto.randomBytes()` |
| Hardcoded keys | In source control forever | Use secret management |
| Encrypt-then-MAC vs MAC-then-encrypt | MAC-then-encrypt has padding oracle attacks | Use AEAD (GCM) which does encrypt-then-MAC internally |
| Not authenticating ciphertext | Ciphertext can be tampered with | Always use authenticated encryption (GCM) |

## Windows-Specific Cryptography Notes

### Windows Certificate Store
Windows uses its own certificate store. Access it properly:

```python
import ssl
import certifi

# Use Windows certificate store via certifi
context = ssl.create_default_context(cafile=certifi.where())

# Or use Windows native certificate store
import ctypes
from ctypes import wintypes

def get_windows_cert_store():
    """Access Windows certificate store."""
    # Use wincertstore or similar library
    import wincertstore
    with wincertstore.CertSystemStore("MY") as store:
        for cert in store.itercerts(usage=wincertstore.SERVER_AUTH):
            yield cert
```

### Windows DPAPI (Data Protection API)
Use Windows DPAPI for encrypting data tied to the user or machine:

```python
import ctypes
from ctypes import wintypes

def encrypt_with_dpapi(data: bytes) -> bytes:
    """Encrypt data using Windows DPAPI."""
    CRYPTPROTECT_UI_FORBIDDEN = 0x01
    
    class DATA_BLOB(ctypes.Structure):
        _fields_ = [
            ("cbData", wintypes.DWORD),
            ("pbData", wintypes.LPBYTE)
        ]
    
    input_blob = DATA_BLOB(len(data), ctypes.cast(data, wintypes.LPBYTE))
    output_blob = DATA_BLOB()
    
    ctypes.windll.crypt32.CryptProtectData(
        ctypes.byref(input_blob),
        None,
        None,
        None,
        None,
        CRYPTPROTECT_UI_FORBIDDEN,
        ctypes.byref(output_blob)
    )
    
    result = ctypes.string_at(output_blob.pbData, output_blob.cbData)
    ctypes.windll.kernel32.LocalFree(output_blob.pbData)
    return result
```

### Windows Credential Manager
Store credentials securely using Windows Credential Manager:

```python
import keyring

# Store password in Windows Credential Manager
keyring.set_password("myapp", "username", "password")

# Retrieve password
password = keyring.get_password("myapp", "username")
```

## Checklist

- [ ] Using established algorithms (AES-GCM, bcrypt/argon2, Ed25519)
- [ ] Using established libraries (cryptography, libsodium, Web Crypto API)
- [ ] Passwords hashed with bcrypt/argon2/scrypt, never SHA or MD5
- [ ] Symmetric encryption uses AES-GCM with unique nonce per key
- [ ] Keys generated with cryptographically secure random
- [ ] Keys stored in secret manager, not hardcoded or in git
- [ ] Key rotation plan exists
- [ ] No custom crypto implementations
- [ ] Windows: Certificate store accessed properly
- [ ] Windows: DPAPI used for user/machine-bound encryption
- [ ] Windows: Credential Manager used for password storage
