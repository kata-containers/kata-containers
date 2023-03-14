use std::io;
use std::cmp;
use std::mem;
use std::fmt;

use buffered_reader::BufferedReader;
use buffered_reader::buffered_reader_generic_read_impl;

use crate::crypto::hash::Digest;
use crate::parse::{Cookie, HashesFor, Hashing};
use crate::Result;
use crate::types::HashAlgorithm;
use crate::types::SignatureType;

const TRACE : bool = false;

/// Controls line-ending normalization during hashing.
///
/// OpenPGP normalizes line endings when signing or verifying text
/// signatures.
#[derive(Clone, Eq)]
pub(crate) enum HashingMode<T> {
    /// Hash for a binary signature.
    ///
    /// The data is hashed as-is.
    Binary(T),

    /// Hash for a text signature.
    ///
    /// The data is hashed with line endings normalized to `\r\n`.
    Text(T),

    /// Like Text, but the last character that we hashed was a '\r'
    /// that we converted to a '\r\n'.
    TextLastWasCr(T),
}

impl<T: std::fmt::Debug> std::fmt::Debug for HashingMode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use self::HashingMode::*;
        match self {
            Binary(t) => write!(f, "Binary({:?})", t),
            Text(t) => write!(f, "Text({:?})", t),
            TextLastWasCr(t) => write!(f, "Text(last was CR, {:?})", t),
        }
    }
}

impl<T: PartialEq> PartialEq for HashingMode<T> {
    fn eq(&self, other: &Self) -> bool {
        use self::HashingMode::*;
        match (self, other) {
            (Binary(s), Binary(o)) => s.eq(o),

            (Text(s), Text(o)) => s.eq(o),
            (TextLastWasCr(s), Text(o)) => s.eq(o),
            (Text(s), TextLastWasCr(o)) => s.eq(o),
            (TextLastWasCr(s), TextLastWasCr(o)) => s.eq(o),

            _ => false,
        }
    }
}

impl<T> HashingMode<T> {
    pub(crate) fn map<U, F: Fn(&T) -> U>(&self, f: F) -> HashingMode<U> {
        use self::HashingMode::*;
        match self {
            Binary(t) => Binary(f(t)),
            Text(t) => Text(f(t)),
            TextLastWasCr(t) => TextLastWasCr(f(t)),
        }
    }

    pub(crate) fn as_ref(&self) -> &T {
        use self::HashingMode::*;
        match self {
            Binary(t) => t,
            Text(t) => t,
            TextLastWasCr(t) => t,
        }
    }

    pub(crate) fn as_mut(&mut self) -> &mut T {
        use self::HashingMode::*;
        match self {
            Binary(t) => t,
            Text(t) => t,
            TextLastWasCr(t) => t,
        }
    }

    pub(crate) fn for_signature(t: T, typ: SignatureType) -> Self {
        if typ == SignatureType::Text {
            HashingMode::Text(t)
        } else {
            HashingMode::Binary(t)
        }
    }

    pub(crate) fn into_inner(self) -> T {
        use self::HashingMode::*;
        match self {
            Binary(t) => t,
            Text(t) => t,
            TextLastWasCr(t) => t,
        }
    }
}

impl<D> HashingMode<D>
    where D: Digest + Clone
{
    /// Updates the given hash context.  When in text mode, normalize
    /// the line endings to "\r\n" on the fly.
    pub(crate) fn update(&mut self, data: &[u8]) {
        if data.is_empty() {
            // This isn't just a short circuit.  It preserves
            // `last_was_cr`, which running through the code would
            // not.
            return;
        }

        let (h, mut last_was_cr) = match self {
            HashingMode::Text(h) => (h, false),
            HashingMode::TextLastWasCr(h) => (h, true),
            HashingMode::Binary(h) => return h.update(data),
        };

        let mut line = data;
        let last_is_cr = line.last() == Some(&b'\r');
        while ! line.is_empty() {
            let mut next = 0;
            for (i, c) in line.iter().cloned().enumerate() {
                match c {
                    b'\n' if last_was_cr => {
                        // We already hash the \n.
                        assert_eq!(i, 0);
                        assert_eq!(next, 0);
                        next = 1;
                        break;
                    },
                    b'\r' | b'\n' => {
                        h.update(&line[..i]);
                        h.update(b"\r\n");
                        next = i + 1;
                        if c == b'\r' && line.get(next) == Some(&b'\n') {
                            next += 1;
                        }
                        break;
                    },
                    _ => (),
                }
                last_was_cr = false;
            }

            if next > 0 {
                line = &line[next..];
            } else {
                h.update(line);
                break;
            }
        }

        match (&mut *self, last_is_cr) {
            (&mut HashingMode::Text(_), false) => {
                // This is the common case.  Getting a crlf that is
                // split across two chunks is extremely rare.  Hence,
                // the clones used to change the variant are rarely
                // needed.
            },
            (&mut HashingMode::Text(ref mut h), true) => {
                *self = HashingMode::TextLastWasCr(h.clone());
            }
            (&mut HashingMode::TextLastWasCr(ref mut h), false) => {
                *self = HashingMode::Text(h.clone());
            },
            (&mut HashingMode::TextLastWasCr(_), true) => (),

            _ => unreachable!("handled above"),
        }
    }
}

pub(crate) struct HashedReader<R: BufferedReader<Cookie>> {
    reader: R,
    cookie: Cookie,
}

impl<R: BufferedReader<Cookie>> fmt::Display for HashedReader<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HashedReader")
    }
}

impl<R: BufferedReader<Cookie>> fmt::Debug for HashedReader<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("HashedReader")
            .field("cookie", &self.cookie)
            .field("reader", &self.reader)
            .finish()
    }
}

impl<R: BufferedReader<Cookie>> HashedReader<R> {
    /// Instantiates a new hashed reader.  `hashes_for` is the hash's
    /// purpose.  `algos` is a list of algorithms for which we should
    /// compute the hash.
    pub fn new(reader: R, hashes_for: HashesFor,
               algos: Vec<HashingMode<HashAlgorithm>>)
            -> Self {
        let mut cookie = Cookie::default();
        for mode in &algos {
            cookie.sig_group_mut().hashes
                .push(mode.map(|algo| algo.context().unwrap())); // XXX: Don't unwrap.
        }
        cookie.hashes_for = hashes_for;

        HashedReader {
            reader,
            cookie,
        }
    }
}

impl Cookie {
    fn hash_update(&mut self, data: &[u8]) {
        let level = self.level.unwrap_or(0);
        let hashes_for = self.hashes_for;
        let ngroups = self.sig_groups.len();

        tracer!(TRACE, "Cookie::hash_update", level);
        t!("({} bytes, {} hashes, enabled: {:?})",
           data.len(), self.sig_group().hashes.len(), self.hashing);

        if self.hashes_for == HashesFor::CleartextSignature {
            return self.hash_update_csf(data);
        }

        // Hash stashed data first.
        if let Some(stashed_data) = self.hash_stash.take() {
            // The stashed data was supposed to be hashed into the
            // then-topmost signature-group's hash, but wasn't,
            // because framing isn't hashed into the topmost signature
            // group.  By the time the parser encountered a new
            // signature group, the data has already been consumed.
            // We fix that here by hashing the stashed data into the
            // former topmost signature-group's hash.
            assert!(ngroups > 1);
            for h in self.sig_groups[ngroups-2].hashes.iter_mut()
            {
                t!("({:?}): group {} {:?} hashing {} stashed bytes.",
                   hashes_for, ngroups-2, h.map(|ctx| ctx.algo()),
                   data.len());

                h.update(&stashed_data);
            }
        }

        if data.is_empty() {
            return;
        }

        if self.hashing == Hashing::Disabled {
            t!("    hash_update: NOT hashing {} bytes: {}.",
               data.len(), crate::fmt::to_hex(data, true));
            return;
        }

        let topmost_group = |i| i == ngroups - 1;
        for (i, sig_group) in self.sig_groups.iter_mut().enumerate() {
            if topmost_group(i) && self.hashing != Hashing::Enabled {
                t!("topmost group {} NOT hashing {} bytes: {}.",
                   i, data.len(), crate::fmt::to_hex(data, true));

                return;
            }

            for h in sig_group.hashes.iter_mut() {
                t!("{:?}: group {} {:?} hashing {} bytes.",
                   hashes_for, i, h.map(|ctx| ctx.algo()), data.len());
                h.update(data);
            }
        }
    }

    fn hash_update_csf(&mut self, mut data: &[u8]) {
        let level = self.level.unwrap_or(0);
        let hashes_for = self.hashes_for;
        let ngroups = self.sig_groups.len();

        assert_eq!(self.hashes_for, HashesFor::CleartextSignature);
        // There is exactly one group.
        assert_eq!(ngroups, 1);

        tracer!(TRACE, "Cookie::hash_update_csf", level);
        t!("Cleartext Signature Framework message");

        // If we stashed half of a \r\n newline away, see if we get
        // the second half now.  If we do, and data is empty then, we
        // return without hashing it.  This is important so that we
        // can avoid hashing the final newline, even if we happen to
        // read it in two invocations of this function.
        if self.hash_stash.as_ref().map(|buf| buf.as_slice() == &b"\r"[..])
            .unwrap_or(false)
            && data.get(0).cloned() == Some(b'\n')
        {
            self.hash_stash.as_mut().expect("checked above").push(b'\n');
            data = &data[1..];
        }

        if data.is_empty() {
            return;
        }

        if self.hashing == Hashing::Disabled {
            t!("    hash_update: NOT hashing {} bytes: {}.",
               data.len(), crate::fmt::to_hex(data, true));
            return;
        }

        // Hash stashed data first.
        if let Some(stashed_data) = self.hash_stash.take() {
            for h in self.sig_groups[0].hashes.iter_mut() {
                t!("{:?}: {:?} hashing {} stashed bytes.",
                   hashes_for, h.map(|ctx| ctx.algo()),
                   stashed_data.len());
                assert!(matches!(h, HashingMode::Text(_)),
                        "CSF transformation uses text signatures");
                h.update(&stashed_data[..]);
            }
        }

        // We hash everything but the last newline.

        // There is exactly one group.
        assert_eq!(ngroups, 1);

        // Compute the length of data that should be hashed.
        // If it ends in a newline, we delay hashing it.
        let l = data.len() - if data.ends_with(b"\r\n") {
            2
        } else if data.ends_with(b"\n") || data.ends_with(b"\r") {
            1
        } else {
            0
        };

        // Hash everything but the last newline now.
        for h in self.sig_groups[0].hashes.iter_mut() {
            t!("{:?}: {:?} hashing {} bytes.",
               hashes_for, h.map(|ctx| ctx.algo()), l);
            assert!(matches!(h, HashingMode::Text(_)),
                    "CSF transformation uses text signatures");
            h.update(&data[..l]);
        }

        // The newline we stash away.  If more text is written
        // later, we will hash it then.  Otherwise, it is
        // implicitly omitted when the filter is dropped.
        if ! data[l..].is_empty() {
            t!("Stashing newline: {:?}", &data[l..]);
            self.hash_stash = Some(data[l..].to_vec());
        }
    }
}

impl<T: BufferedReader<Cookie>> io::Read for HashedReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        buffered_reader_generic_read_impl(self, buf)
    }
}

// Wrap a BufferedReader so that any data that is consumed is added to
// the hash.
impl<R: BufferedReader<Cookie>>
        BufferedReader<Cookie> for HashedReader<R> {
    fn buffer(&self) -> &[u8] {
        self.reader.buffer()
    }

    fn data(&mut self, amount: usize) -> io::Result<&[u8]> {
        self.reader.data(amount)
    }

    fn data_hard(&mut self, amount: usize) -> io::Result<&[u8]> {
        self.reader.data_hard(amount)
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        // We need to take the state rather than get a mutable
        // reference to it, because self.reader.buffer() requires a
        // reference as well.
        let mut state = self.cookie_set(Cookie::default());

        {
            // The inner buffered reader must return at least `amount`
            // bytes, because the caller can't `consume(amount)` if
            // the internal buffer doesn't have at least that many
            // bytes.
            let data = self.reader.buffer();
            assert!(data.len() >= amount);
            state.hash_update(&data[..amount]);
        }

        self.cookie_set(state);

        self.reader.consume(amount)
    }

    fn data_consume(&mut self, amount: usize) -> io::Result<&[u8]> {
        // See consume() for an explanation of the following
        // acrobatics.

        let mut state = self.cookie_set(Cookie::default());

        let got = {
            let data = self.reader.data(amount)?;
            let data = &data[..cmp::min(data.len(), amount)];
            state.hash_update(data);
            data.len()
        };

        self.cookie_set(state);

        if let Ok(data) = self.reader.data_consume(amount) {
            assert!(data.len() >= got);
            Ok(data)
        } else {
            panic!("reader.data_consume() returned less than reader.data()!");
        }
    }

    fn data_consume_hard(&mut self, amount: usize) -> io::Result<&[u8]> {
        // See consume() for an explanation of the following
        // acrobatics.

        let mut state = self.cookie_set(Cookie::default());

        {
            let data = self.reader.data_hard(amount)?;
            assert!(data.len() >= amount);
            state.hash_update(&data[..amount]);
        }

        self.cookie_set(state);

        let result = self.reader.data_consume(amount);
        assert!(result.is_ok());
        result
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<Cookie>> {
        Some(&mut self.reader)
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<Cookie>> {
        Some(&self.reader)
    }

    fn into_inner<'b>(self: Box<Self>)
            -> Option<Box<dyn BufferedReader<Cookie> + 'b>>
            where Self: 'b {
        Some(self.reader.as_boxed())
    }

    fn cookie_set(&mut self, cookie: Cookie) -> Cookie {
        mem::replace(&mut self.cookie, cookie)
    }

    fn cookie_ref(&self) -> &Cookie {
        &self.cookie
    }

    fn cookie_mut(&mut self) -> &mut Cookie {
        &mut self.cookie
    }
}

/// Hashes the given buffered reader.
///
/// This can be used to verify detached signatures.  For a more
/// convenient method, see [`DetachedVerifier`].
///
///  [`DetachedVerifier`]: crate::parse::stream::DetachedVerifier
pub(crate) fn hash_buffered_reader<R>(reader: R,
                                      algos: &[HashingMode<HashAlgorithm>])
    -> Result<Vec<HashingMode<Box<dyn crate::crypto::hash::Digest>>>>
    where R: BufferedReader<crate::parse::Cookie>,
{
    let mut reader
        = HashedReader::new(reader, HashesFor::Signature, algos.to_vec());

    // Hash all of the data.
    reader.drop_eof()?;

    let hashes =
        mem::take(&mut reader.cookie_mut().sig_group_mut().hashes);
    Ok(hashes)
}

#[cfg(test)]
mod test {
    use super::*;

    use buffered_reader::BufferedReader;

    #[test]
    fn hash_test_1() {
        use std::collections::HashMap;
        struct Test<'a> {
            data: &'a [u8],
            expected: HashMap<HashAlgorithm, &'a str>,
        }

        let tests = [
            Test {
                data: &b"foobar\n"[..],
                expected: [
                    (HashAlgorithm::SHA1,
                     "988881adc9fc3655077dc2d4d757d480b5ea0e11"),
                ].iter().cloned().collect(),
            },
            Test {
                data: &b"0123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789\n"[..],
                expected: [
                    (HashAlgorithm::SHA1,
                     "1d12c55b3a85daab4776a1df41a8f30ada099e11"),
                    (HashAlgorithm::SHA224,
                     "a4c1bde77c682a0e9e30c6afdd1ece2397ffeec61dde2a0eaa23191e"),
                    (HashAlgorithm::SHA256,
                    "151a1d51a1870dc244f07f4844f46ee65fae19a8efeb60b203a074aff899e27d"),
                    (HashAlgorithm::SHA384,
                    "5bea68c8c696bbed95e152d61c446ad0e05bf68f7df39cbfeae568bee6f6691c840fb1d5dd2599737b08dbb33eed344b"),
                    (HashAlgorithm::SHA512,
                     "5fa032487774082af5cc833c2db5f943e31cc75cd2bfaa7d9bbd0ccabf5403b6dbcb484254727a524588f20e9ef336d8ce8533332c5ac1b9d50af3003a0da8d8"),
                ].iter().filter(|(hash, _)| hash.is_supported()).cloned().collect(),
            },
        ];

        for test in tests.iter() {
            let reader
                = buffered_reader::Generic::with_cookie(
                    test.data, None, Default::default());
            let mut reader
                = HashedReader::new(reader, HashesFor::MDC,
                                    test.expected.keys().cloned()
                                    .map(HashingMode::Binary)
                                    .collect());

            assert_eq!(reader.steal_eof().unwrap(), test.data);

            let cookie = reader.cookie_mut();

            let mut hashes = std::mem::take(&mut cookie.sig_group_mut().hashes);
            for mode in hashes.iter_mut() {
                let hash = mode.as_mut();
                let algo = hash.algo();
                let mut digest = vec![0u8; hash.digest_size()];
                let _ = hash.digest(&mut digest);

                assert_eq!(digest,
                           &crate::fmt::from_hex(test.expected.get(&algo)
                                                    .unwrap(), true)
                           .unwrap()[..],
                           "Algo: {:?}", algo);
            }
        }
    }

    #[test]
    fn hash_update_text() -> crate::Result<()> {
        for text in &[
            "one\r\ntwo\r\nthree",
            "one\ntwo\nthree",
            "one\rtwo\rthree",
            "one\ntwo\r\nthree",
        ] {
            for chunk_size in &[ text.len(), 1 ] {
                let mut ctx
                    = HashingMode::Text(HashAlgorithm::SHA256.context()?);
                for chunk in text.as_bytes().chunks(*chunk_size) {
                    ctx.update(chunk);
                }
                let mut ctx = ctx.into_inner();
                let mut digest = vec![0; ctx.digest_size()];
                let _ = ctx.digest(&mut digest);
                assert_eq!(
                    &crate::fmt::hex::encode(&digest),
                    "5536758151607BB81CE8D6F49189B2E84763DA9EA84965AB7327E704DAE415EB",
                    "{:?}, chunk size: {}", text, chunk_size);
            }
        }
        Ok(())
    }

    #[test]
    fn hash_reader_test() {
        use std::collections::HashMap;

        let expected: HashMap<HashAlgorithm, &str> = [
            (HashAlgorithm::SHA1, "7945E3DA269C25C04F9EF435A5C0F25D9662C771"),
            (HashAlgorithm::SHA512, "DDE60DB05C3958AF1E576CD006A7F3D2C343DD8C\
                                     8DECE789A15D148DF90E6E0D1454DE734F834350\
                                     2CA93759F22C8F6221BE35B6BDE9728BD12D2891\
                                     22437CB1"),
        ].iter().cloned().collect();

        let reader
            = buffered_reader::Generic::with_cookie(
                std::io::Cursor::new(crate::tests::manifesto()),
                None, Default::default());
        let result =
            hash_buffered_reader(
                reader,
                &expected.keys().cloned()
                    .map(HashingMode::Binary).
                    collect::<Vec<_>>())
            .unwrap();

        for mut mode in result.into_iter() {
            let hash = mode.as_mut();
            let algo = hash.algo();
            let mut digest = vec![0u8; hash.digest_size()];
            let _ = hash.digest(&mut digest);

            assert_eq!(*expected.get(&algo).unwrap(),
                       &crate::fmt::to_hex(&digest[..], false));
        }
    }
}
