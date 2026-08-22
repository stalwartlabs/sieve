/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

#[cfg(any(test, feature = "serde"))]
pub(crate) mod as_string_vec_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub fn serialize<S: Serializer>(
        value: &Arc<[Arc<str>]>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(value.iter().map(|value| value.as_ref()))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<[Arc<str>]>, D::Error> {
        Vec::<String>::deserialize(deserializer)
            .map(|values| values.into_iter().map(Arc::from).collect())
    }
}

#[cfg(feature = "rkyv")]
pub use archive::ArchiveError;

#[cfg(feature = "rkyv")]
mod archive {
    use crate::{Compiler, Sieve};
    use rkyv::{Archive, rancor};
    use std::fmt::{Display, Formatter};

    const _: () = assert!(
        Compiler::VERSION <= 0x7f,
        "Compiler::VERSION no longer fits in the trailing header byte, switch to LEB128."
    );

    #[derive(Debug)]
    pub enum ArchiveError {
        Truncated,
        UnsupportedVersion(u8),
        Archive(rancor::Error),
    }

    impl Display for ArchiveError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                ArchiveError::Truncated => f.write_str("Truncated Sieve script"),
                ArchiveError::UnsupportedVersion(version) => write!(
                    f,
                    "Sieve script was compiled by version {version}, expected version {}",
                    Compiler::VERSION
                ),
                ArchiveError::Archive(err) => write!(f, "Corrupted Sieve script: {err}"),
            }
        }
    }

    impl std::error::Error for ArchiveError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                ArchiveError::Archive(err) => Some(err),
                _ => None,
            }
        }
    }

    const MIN_ARCHIVE_SIZE: usize = std::mem::size_of::<<Sieve as Archive>::Archived>();

    impl Sieve {
        pub fn to_bytes(&self) -> Result<Vec<u8>, ArchiveError> {
            let mut bytes = rkyv::api::high::to_bytes_in::<_, rkyv::rancor::Error>(
                self,
                Vec::with_capacity(MIN_ARCHIVE_SIZE + 1),
            )
            .map_err(ArchiveError::Archive)?;
            bytes.push(Compiler::VERSION as u8);
            Ok(bytes)
        }

        pub fn from_bytes(bytes: &[u8]) -> Result<Self, ArchiveError> {
            let Some((version, payload)) = bytes.split_last() else {
                return Err(ArchiveError::Truncated);
            };
            if *version != Compiler::VERSION as u8 {
                return Err(ArchiveError::UnsupportedVersion(*version));
            } else if payload.len() < MIN_ARCHIVE_SIZE {
                return Err(ArchiveError::Truncated);
            }
            rkyv::from_bytes::<Sieve, rancor::Error>(payload).map_err(ArchiveError::Archive)
        }

        #[allow(clippy::missing_safety_doc)]
        pub unsafe fn from_bytes_unchecked(bytes: &[u8]) -> Result<Self, ArchiveError> {
            let Some((version, payload)) = bytes.split_last() else {
                return Err(ArchiveError::Truncated);
            };
            if *version != Compiler::VERSION as u8 {
                return Err(ArchiveError::UnsupportedVersion(*version));
            }

            if payload.len() >= MIN_ARCHIVE_SIZE {
                unsafe { rkyv::from_bytes_unchecked::<Sieve, rancor::Error>(payload) }
                    .map_err(ArchiveError::Archive)
            } else {
                Err(ArchiveError::Truncated)
            }
        }
    }
}

#[cfg(feature = "rkyv")]
pub struct AsStringVec;

#[cfg(feature = "rkyv")]
const _: () = {
    use rkyv::{
        Archive, Place, Serialize,
        rancor::{Fallible, Source},
        ser::{Allocator, Writer},
        string::{ArchivedString, StringResolver},
        vec::{ArchivedVec, VecResolver},
        with::{ArchiveWith, DeserializeWith, SerializeWith},
    };
    use std::sync::Arc;

    struct StrRef<'x>(&'x str);

    impl Archive for StrRef<'_> {
        type Archived = ArchivedString;
        type Resolver = StringResolver;

        fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
            ArchivedString::resolve_from_str(self.0, resolver, out);
        }
    }

    impl<S> Serialize<S> for StrRef<'_>
    where
        S: Fallible + Writer + ?Sized,
        S::Error: Source,
    {
        fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
            ArchivedString::serialize_from_str(self.0, serializer)
        }
    }

    impl ArchiveWith<Arc<[Arc<str>]>> for AsStringVec {
        type Archived = ArchivedVec<ArchivedString>;
        type Resolver = VecResolver;

        fn resolve_with(
            field: &Arc<[Arc<str>]>,
            resolver: Self::Resolver,
            out: Place<Self::Archived>,
        ) {
            ArchivedVec::resolve_from_len(field.len(), resolver, out);
        }
    }

    impl<S> SerializeWith<Arc<[Arc<str>]>, S> for AsStringVec
    where
        S: Fallible + Allocator + Writer + ?Sized,
        S::Error: Source,
    {
        fn serialize_with(
            field: &Arc<[Arc<str>]>,
            serializer: &mut S,
        ) -> Result<Self::Resolver, S::Error> {
            ArchivedVec::serialize_from_iter(
                field.iter().map(|value| StrRef(value.as_ref())),
                serializer,
            )
        }
    }

    impl<D> DeserializeWith<ArchivedVec<ArchivedString>, Arc<[Arc<str>]>, D> for AsStringVec
    where
        D: Fallible + ?Sized,
    {
        fn deserialize_with(
            field: &ArchivedVec<ArchivedString>,
            _: &mut D,
        ) -> Result<Arc<[Arc<str>]>, D::Error> {
            Ok(field
                .iter()
                .map(|value| Arc::from(value.as_str()))
                .collect())
        }
    }
};
