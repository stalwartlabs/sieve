/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use mail_parser::HeaderName;

pub(crate) const HEADER_OTHER: u8 = 0xff;

macro_rules! header_ids {
    ($($id:literal => $variant:ident),* $(,)?) => {
        pub(crate) fn header_id(name: &HeaderName<'_>) -> Option<u8> {
            match name {
                $(HeaderName::$variant => Some($id),)*
                _ => None,
            }
        }

        pub(crate) fn header_from_id(id: u8) -> Option<HeaderName<'static>> {
            match id {
                $($id => Some(HeaderName::$variant),)*
                _ => None,
            }
        }
    };
}

header_ids! {
    0 => Subject,
    1 => From,
    2 => To,
    3 => Cc,
    4 => Date,
    5 => Bcc,
    6 => ReplyTo,
    7 => Sender,
    8 => Comments,
    9 => InReplyTo,
    10 => Keywords,
    11 => Received,
    12 => MessageId,
    13 => References,
    14 => ReturnPath,
    15 => MimeVersion,
    16 => ContentDescription,
    17 => ContentId,
    18 => ContentLanguage,
    19 => ContentLocation,
    20 => ContentTransferEncoding,
    21 => ContentType,
    22 => ContentDisposition,
    23 => ResentTo,
    24 => ResentFrom,
    25 => ResentBcc,
    26 => ResentCc,
    27 => ResentSender,
    28 => ResentDate,
    29 => ResentMessageId,
    30 => ListArchive,
    31 => ListHelp,
    32 => ListId,
    33 => ListOwner,
    34 => ListPost,
    35 => ListSubscribe,
    36 => ListUnsubscribe,
    37 => DkimSignature,
    38 => ArcAuthenticationResults,
    39 => ArcMessageSignature,
    40 => ArcSeal,
    41 => Dkim2Signature,
    42 => MessageInstance,
    43 => AcceptLanguage,
    44 => AlternateRecipient,
    45 => ArchivedAt,
    46 => AuthenticationResults,
    47 => AutoSubmitted,
    48 => Autoforwarded,
    49 => Autosubmitted,
    50 => ContentAlternative,
    51 => ContentDuration,
    52 => ContentFeatures,
    53 => ContentMd5,
    54 => ContentTranslationType,
    55 => Conversion,
    56 => ConversionWithLoss,
    57 => DlExpansionHistory,
    58 => DeferredDelivery,
    59 => DeliveryDate,
    60 => DiscardedX400IpmsExtensions,
    61 => DiscardedX400MtsExtensions,
    62 => DiscloseRecipients,
    63 => DispositionNotificationOptions,
    64 => DispositionNotificationTo,
    65 => DowngradedFinalRecipient,
    66 => DowngradedInReplyTo,
    67 => DowngradedMessageId,
    68 => DowngradedOriginalRecipient,
    69 => DowngradedReferences,
    70 => Encoding,
    71 => Expires,
    72 => GenerateDeliveryReport,
    73 => HpOuter,
    74 => Importance,
    75 => IncompleteCopy,
    76 => Language,
    77 => LatestDeliveryTime,
    78 => ListUnsubscribePost,
    79 => MessageContext,
    80 => MessageType,
    81 => MmhsExemptedAddress,
    82 => MmhsExtendedAuthorisationInfo,
    83 => MmhsSubjectIndicatorCodes,
    84 => MmhsHandlingInstructions,
    85 => MmhsMessageInstructions,
    86 => MmhsCodressMessageIndicator,
    87 => MmhsOriginatorReference,
    88 => MmhsPrimaryPrecedence,
    89 => MmhsCopyPrecedence,
    90 => MmhsMessageType,
    91 => MmhsOtherRecipientsIndicatorTo,
    92 => MmhsOtherRecipientsIndicatorCc,
    93 => MmhsAcp127MessageIdentifier,
    94 => MmhsOriginatorPlad,
    95 => MtPriority,
    96 => Organization,
    97 => OriginalEncodedInformationTypes,
    98 => OriginalFrom,
    99 => OriginalMessageId,
    100 => OriginalRecipient,
    101 => OriginatorReturnAddress,
    102 => OriginalSubject,
    103 => PicsLabel,
    104 => PreventNonDeliveryReport,
    105 => Priority,
    106 => ReceivedSpf,
    107 => ReplyBy,
    108 => RequireRecipientValidSince,
    109 => Sensitivity,
    110 => Solicitation,
    111 => Supersedes,
    112 => TlsReportDomain,
    113 => TlsReportSubmitter,
    114 => TlsRequired,
    115 => VbrInfo,
    116 => X400ContentIdentifier,
    117 => X400ContentReturn,
    118 => X400ContentType,
    119 => X400MtsIdentifier,
    120 => X400Originator,
    121 => X400Received,
    122 => X400Recipients,
    123 => X400Trace,
    124 => ApparentlyTo,
    125 => Author,
    126 => CfblAddress,
    127 => CfblFeedbackId,
    128 => DeliveredTo,
    129 => EdiintFeatures,
    130 => EesstVersion,
    131 => ErrorsTo,
    132 => Face,
    133 => FormSub,
    134 => JabberId,
    135 => MmhsAuthorizingUsers,
    136 => Privicon,
    137 => SioLabel,
    138 => SioLabelHistory,
    139 => WrongRecipient,
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER_ID_COUNT: u8 = 140;

    #[test]
    fn header_ids_round_trip() {
        for id in 0..HEADER_ID_COUNT {
            let name = header_from_id(id).unwrap_or_else(|| panic!("missing header id {id}"));
            assert_eq!(header_id(&name), Some(id), "{name:?}");
            assert_eq!(
                HeaderName::parse(name.as_str()),
                Some(name.clone()),
                "{name:?}"
            );
        }
        for id in HEADER_ID_COUNT..=HEADER_OTHER {
            assert_eq!(header_from_id(id), None);
        }
        assert_eq!(header_id(&HeaderName::Other("X-Test".into())), None);
    }
}
