use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;
#[cfg(verus_keep_ghost)]
use vstd::utf8::{is_ascii_chars, is_ascii_chars_encode_utf8};

verus! {

pub open spec fn server_label() -> Seq<u8> {
    "post-server-proof-0\0".spec_bytes()
}

pub open spec fn account_label() -> Seq<u8> {
    "post-account-proof-0\0".spec_bytes()
}

pub open spec fn server_transcript(address: Seq<u8>, client: Seq<u8>, server: Seq<u8>) -> Seq<u8> {
    server_label() + address + seq![0u8] + client + server
}

pub open spec fn account_transcript(
    address: Seq<u8>,
    key: Seq<u8>,
    client: Seq<u8>,
    server: Seq<u8>,
) -> Seq<u8> {
    account_label() + address + seq![0u8] + key + client + server
}

proof fn lemma_the_labels()
    ensures
        server_label().len() == 20,
        server_label()[5] == b's',
        account_label().len() == 21,
        account_label()[5] == b'a',
{
    reveal_strlit("post-server-proof-0\0");
    reveal_strlit("post-account-proof-0\0");
    assert(is_ascii_chars("post-server-proof-0\0"@));
    assert(is_ascii_chars("post-account-proof-0\0"@));
    is_ascii_chars_encode_utf8("post-server-proof-0\0"@);
    is_ascii_chars_encode_utf8("post-account-proof-0\0"@);
}

pub proof fn lemma_a_server_transcript_gives_its_parts(
    address: Seq<u8>,
    client: Seq<u8>,
    server: Seq<u8>,
    other_address: Seq<u8>,
    other_client: Seq<u8>,
    other_server: Seq<u8>,
)
    requires
        client.len() == 32,
        server.len() == 32,
        other_client.len() == 32,
        other_server.len() == 32,
        server_transcript(address, client, server) == server_transcript(
            other_address,
            other_client,
            other_server,
        ),
    ensures
        address == other_address,
        client == other_client,
        server == other_server,
{
    let label = server_label();
    let transcript = server_transcript(address, client, server);
    let other = server_transcript(other_address, other_client, other_server);
    let start = label.len() as int;
    let end = start + address.len();
    assert(transcript.len() == start + address.len() + 65);
    assert(other.len() == start + other_address.len() + 65);
    assert(address =~= transcript.subrange(start, end));
    assert(other_address =~= other.subrange(start, end));
    assert(client =~= transcript.subrange(end + 1, end + 33));
    assert(other_client =~= other.subrange(end + 1, end + 33));
    assert(server =~= transcript.subrange(end + 33, end + 65));
    assert(other_server =~= other.subrange(end + 33, end + 65));
}

pub proof fn lemma_an_account_transcript_gives_its_parts(
    address: Seq<u8>,
    key: Seq<u8>,
    client: Seq<u8>,
    server: Seq<u8>,
    other_address: Seq<u8>,
    other_key: Seq<u8>,
    other_client: Seq<u8>,
    other_server: Seq<u8>,
)
    requires
        key.len() == 32,
        client.len() == 32,
        server.len() == 32,
        other_key.len() == 32,
        other_client.len() == 32,
        other_server.len() == 32,
        account_transcript(address, key, client, server) == account_transcript(
            other_address,
            other_key,
            other_client,
            other_server,
        ),
    ensures
        address == other_address,
        key == other_key,
        client == other_client,
        server == other_server,
{
    let label = account_label();
    let transcript = account_transcript(address, key, client, server);
    let other = account_transcript(other_address, other_key, other_client, other_server);
    let start = label.len() as int;
    let end = start + address.len();
    assert(transcript.len() == start + address.len() + 97);
    assert(other.len() == start + other_address.len() + 97);
    assert(address =~= transcript.subrange(start, end));
    assert(other_address =~= other.subrange(start, end));
    assert(key =~= transcript.subrange(end + 1, end + 33));
    assert(other_key =~= other.subrange(end + 1, end + 33));
    assert(client =~= transcript.subrange(end + 33, end + 65));
    assert(other_client =~= other.subrange(end + 33, end + 65));
    assert(server =~= transcript.subrange(end + 65, end + 97));
    assert(other_server =~= other.subrange(end + 65, end + 97));
}

pub proof fn lemma_a_server_transcript_is_not_an_account_transcript(
    address: Seq<u8>,
    client: Seq<u8>,
    server: Seq<u8>,
    other_address: Seq<u8>,
    other_key: Seq<u8>,
    other_client: Seq<u8>,
    other_server: Seq<u8>,
)
    ensures
        server_transcript(address, client, server) != account_transcript(
            other_address,
            other_key,
            other_client,
            other_server,
        ),
{
    lemma_the_labels();
    assert(server_transcript(address, client, server)[5] == b's');
    assert(account_transcript(other_address, other_key, other_client, other_server)[5] == b'a');
}

pub(crate) fn write_server_transcript(address: &str, client: &[u8; 32], server: &[u8; 32]) -> (transcript:
    Vec<u8>)
    ensures
        transcript@ == server_transcript(address.spec_bytes(), client@, server@),
{
    let mut transcript = Vec::new();
    transcript.extend_from_slice("post-server-proof-0\0".as_bytes());
    transcript.extend_from_slice(address.as_bytes());
    transcript.push(0);
    transcript.extend_from_slice(client.as_slice());
    transcript.extend_from_slice(server.as_slice());
    assert(transcript@ =~= server_transcript(address.spec_bytes(), client@, server@));
    transcript
}

pub(crate) fn write_account_transcript(
    address: &str,
    key: &[u8; 32],
    client: &[u8; 32],
    server: &[u8; 32],
) -> (transcript: Vec<u8>)
    ensures
        transcript@ == account_transcript(address.spec_bytes(), key@, client@, server@),
{
    let mut transcript = Vec::new();
    transcript.extend_from_slice("post-account-proof-0\0".as_bytes());
    transcript.extend_from_slice(address.as_bytes());
    transcript.push(0);
    transcript.extend_from_slice(key.as_slice());
    transcript.extend_from_slice(client.as_slice());
    transcript.extend_from_slice(server.as_slice());
    assert(transcript@ =~= account_transcript(address.spec_bytes(), key@, client@, server@));
    transcript
}

} // verus!
