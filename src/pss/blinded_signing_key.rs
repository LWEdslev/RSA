use super::{get_pss_signature_algo_id, sign_digest, Signature, VerifyingKey};
use crate::{Result, RsaPrivateKey};
use const_oid::AssociatedOid;
use core::marker::PhantomData;
use digest::{Digest, FixedOutputReset};
use pkcs8::{
    spki::{
        der::AnyRef, AlgorithmIdentifierOwned, AlgorithmIdentifierRef,
        AssociatedAlgorithmIdentifier, DynSignatureAlgorithmIdentifier,
    },
    EncodePrivateKey, SecretDocument,
};
use rand_core::CryptoRngCore;
use signature::{
    hazmat::RandomizedPrehashSigner, Keypair, RandomizedDigestSigner, RandomizedSigner,
};
use zeroize::ZeroizeOnDrop;
#[cfg(feature = "serde")]
use {
    pkcs8::PrivateKeyInfo,
    serdect::serde::{de, ser, Deserialize, Serialize},
    spki::der::Decode,
};
/// Signing key for producing "blinded" RSASSA-PSS signatures as described in
/// [draft-irtf-cfrg-rsa-blind-signatures](https://datatracker.ietf.org/doc/draft-irtf-cfrg-rsa-blind-signatures/).
#[derive(Debug, Clone)]
pub struct BlindedSigningKey<D>
where
    D: Digest,
{
    inner: RsaPrivateKey,
    salt_len: usize,
    phantom: PhantomData<D>,
}

impl<D> BlindedSigningKey<D>
where
    D: Digest,
{
    /// Create a new RSASSA-PSS signing key which produces "blinded"
    /// signatures.
    /// Digest output size is used as a salt length.
    pub fn new(key: RsaPrivateKey) -> Self {
        Self::new_with_salt_len(key, <D as Digest>::output_size())
    }

    /// Create a new RSASSA-PSS signing key which produces "blinded"
    /// signatures with a salt of the given length.
    pub fn new_with_salt_len(key: RsaPrivateKey, salt_len: usize) -> Self {
        Self {
            inner: key,
            salt_len,
            phantom: Default::default(),
        }
    }

    /// Create a new random RSASSA-PSS signing key which produces "blinded"
    /// signatures.
    /// Digest output size is used as a salt length.
    pub fn random<R: CryptoRngCore + ?Sized>(rng: &mut R, bit_size: usize) -> Result<Self> {
        Self::random_with_salt_len(rng, bit_size, <D as Digest>::output_size())
    }

    /// Create a new random RSASSA-PSS signing key which produces "blinded"
    /// signatures with a salt of the given length.
    pub fn random_with_salt_len<R: CryptoRngCore + ?Sized>(
        rng: &mut R,
        bit_size: usize,
        salt_len: usize,
    ) -> Result<Self> {
        Ok(Self {
            inner: RsaPrivateKey::new(rng, bit_size)?,
            salt_len,
            phantom: Default::default(),
        })
    }

    /// Return specified salt length for this key
    pub fn salt_len(&self) -> usize {
        self.salt_len
    }
}

//
// `*Signer` trait impls
//

impl<D> RandomizedSigner<Signature> for BlindedSigningKey<D>
where
    D: Digest + FixedOutputReset,
{
    fn try_sign_with_rng(
        &self,
        rng: &mut impl CryptoRngCore,
        msg: &[u8],
    ) -> signature::Result<Signature> {
        sign_digest::<_, D>(rng, true, &self.inner, &D::digest(msg), self.salt_len)?
            .as_slice()
            .try_into()
    }
}

impl<D> RandomizedDigestSigner<D, Signature> for BlindedSigningKey<D>
where
    D: Digest + FixedOutputReset,
{
    fn try_sign_digest_with_rng(
        &self,
        rng: &mut impl CryptoRngCore,
        digest: D,
    ) -> signature::Result<Signature> {
        sign_digest::<_, D>(rng, true, &self.inner, &digest.finalize(), self.salt_len)?
            .as_slice()
            .try_into()
    }
}

impl<D> RandomizedPrehashSigner<Signature> for BlindedSigningKey<D>
where
    D: Digest + FixedOutputReset,
{
    fn sign_prehash_with_rng(
        &self,
        rng: &mut impl CryptoRngCore,
        prehash: &[u8],
    ) -> signature::Result<Signature> {
        sign_digest::<_, D>(rng, true, &self.inner, prehash, self.salt_len)?
            .as_slice()
            .try_into()
    }
}

//
// Other trait impls
//

impl<D> AsRef<RsaPrivateKey> for BlindedSigningKey<D>
where
    D: Digest,
{
    fn as_ref(&self) -> &RsaPrivateKey {
        &self.inner
    }
}

impl<D> AssociatedAlgorithmIdentifier for BlindedSigningKey<D>
where
    D: Digest,
{
    type Params = AnyRef<'static>;

    const ALGORITHM_IDENTIFIER: AlgorithmIdentifierRef<'static> = pkcs1::ALGORITHM_ID;
}

impl<D> DynSignatureAlgorithmIdentifier for BlindedSigningKey<D>
where
    D: Digest + AssociatedOid,
{
    fn signature_algorithm_identifier(&self) -> pkcs8::spki::Result<AlgorithmIdentifierOwned> {
        get_pss_signature_algo_id::<D>(self.salt_len as u8)
    }
}

impl<D> EncodePrivateKey for BlindedSigningKey<D>
where
    D: Digest,
{
    fn to_pkcs8_der(&self) -> pkcs8::Result<SecretDocument> {
        self.inner.to_pkcs8_der()
    }
}

impl<D> From<RsaPrivateKey> for BlindedSigningKey<D>
where
    D: Digest,
{
    fn from(key: RsaPrivateKey) -> Self {
        Self::new(key)
    }
}

impl<D> From<BlindedSigningKey<D>> for RsaPrivateKey
where
    D: Digest,
{
    fn from(key: BlindedSigningKey<D>) -> Self {
        key.inner
    }
}

impl<D> Keypair for BlindedSigningKey<D>
where
    D: Digest,
{
    type VerifyingKey = VerifyingKey<D>;
    fn verifying_key(&self) -> Self::VerifyingKey {
        VerifyingKey {
            inner: self.inner.to_public_key(),
            salt_len: self.salt_len,
            phantom: Default::default(),
        }
    }
}

impl<D> ZeroizeOnDrop for BlindedSigningKey<D> where D: Digest {}

#[cfg(feature = "serde")]
impl<D> Serialize for BlindedSigningKey<D>
where
    D: Digest,
{
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let der = self.inner.to_pkcs8_der().map_err(ser::Error::custom)?;
        serdect::slice::serialize_hex_lower_or_bin(&der.as_bytes(), serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, D> Deserialize<'de> for BlindedSigningKey<D>
where
    D: Digest + AssociatedOid,
{
    fn deserialize<De>(deserializer: De) -> core::result::Result<Self, De::Error>
    where
        De: serde::Deserializer<'de>,
    {
        let der_bytes = serdect::slice::deserialize_hex_or_bin_vec(deserializer)?;
        let pki = PrivateKeyInfo::from_der(&der_bytes).map_err(de::Error::custom)?;
        RsaPrivateKey::try_from(pki)
            .map_err(de::Error::custom)
            .map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde() {
        use rand_chacha::{rand_core::SeedableRng, ChaCha8Rng};
        use sha2::Sha256;

        let mut rng = ChaCha8Rng::from_seed([42; 32]);
        let signing_key = BlindedSigningKey::<Sha256>::new(RsaPrivateKey::new(&mut rng, 64).expect("failed to generate key"));

        let ser_signing_key = serde_json::to_string(&signing_key).expect("unable to serialize signing key");
        let deser_signing_key = serde_json::from_str::<BlindedSigningKey<Sha256>>(&ser_signing_key).expect("unable to deserialize signing key");
        
        assert_eq!(signing_key.inner, deser_signing_key.inner);
        assert_eq!(signing_key.salt_len, deser_signing_key.salt_len);
    }
}