# Which words does JMdict know that IPADIC cuts apart?

**Date:** 2026-09-16
**Tool:** `crates/jparser/examples/shatter_sweep.rs` (read-only)
**Question:** `いちゃいちゃ` is in the index and the engine shatters it. How
many words are like that, and is a Vibrato user lexicon the fix?

## 1. Why the tokenizer and not the cost model

The cost model cannot fix this class, and
[`2026-09-16-per-match-cost-spike.md`](2026-09-16-per-match-cost-spike.md)
proves it: no per-character scheme can, and the per-match constant that would
reopens §4.3's particle-absorbing merges.

But the fix already exists in the engine, and it is the boundary constraint:

```
美少女      hints ON : 美少女          hints OFF: 美|少女
いちゃいちゃ  hints ON : い|ちゃい|ちゃ     hints OFF: い|ちゃい|ちゃ
```

`美少女` is the same shape — a non-COMMON word whose pieces are COMMON — and
it survives hints-on **only** because IPADIC holds it as one token, so
`is_allowed` forbids any span from starting or ending inside it. `いちゃいちゃ`
loses because IPADIC cuts it into four tokens, `い | ちゃ | い | ちゃ`.

So this is a data gap in the hint dictionary, not a flaw in the cost model,
and Vibrato takes a user lexicon
(`Dictionary::reset_user_lexicon_from_reader`, MeCab-format CSV).

## 2. The sweep

Every kana-only JMdict headword of four or more characters, excluding entries
JMdict tags `exp` — a phrase *should* come apart into its component words, so
a split there is not damage. Each word is tokenized, then parsed with hints,
and counted as shattered when the parse does not return it as one matched
span.

```
kana-only headwords considered:        44545
  the tokenizer cuts inside:           14771
  and the engine then shatters:        12643
    shortest piece 1:    878   of which all-hiragana: 591
    shortest piece 2:   2885   of which all-hiragana: 365
    shortest piece 3+:  8880   (a decomposition into real words)
```

## 3. The headline: a bulk import would be wrong

**8,880 of the 12,643 — seven in ten — shatter into pieces of three or more
characters**, and that is usually the better answer:

```
アプリケーションサービスプロバイダー -> アプリケーション | サービス | プロバイダー
インターネットサービスプロバイダー   -> インターネット | サービス | プロバイダー
```

Three lookupable words beat one long unglossed blob, which is exactly the
judgement `relax::MIN_PIECE` already encodes. Importing all 12,643 would glue
these back together and make the reader's experience worse.

The damage is the tail that shatters into **fragments**: 3,763 words with a
one- or two-character piece, of which **956 are all-hiragana single lexical
items**. That is the candidate set, and it looks like this:

```
しっちゃかめっちゃか -> しっ | ちゃ | か | めっちゃ | か
いないいないばあ     -> い | ない | い | ない | ば | あ
すっとこどっこい     -> すっ | とこ | どっこい
まっくろくろすけ     -> まっくろく | ろ | すけ
うちなんちゅう       -> うち | な | ん | ちゅう
いちゃいちゃ         -> い | ちゃい | ちゃ
```

## 4. Why this route is safe, structurally

Phase 4 exists because a hint token JMdict cannot cover destroys its region —
so adding hint entries is normally a risk, and the instinct is to gate it
hard.

**That failure mode cannot occur for this list.** Every word in it is in
JMdict *by construction* — being in JMdict is how it was found. Adding it to
the user lexicon makes IPADIC emit it as one token, which **removes** interior
boundaries rather than adding any, and the token that appears is matchable by
definition. The veto can only relax.

The residual risks are ordinary rather than structural: a user lexicon entry
needs a plausible cost and part of speech or Vibrato's own path costs will
misbehave, and a word that is genuinely ambiguous in context (`いない` inside
`いないいないばあ`) now has a long token competing with the short reading.
Both are measurable against the gates already standing.

## 5. Not done

No user lexicon is written and no behaviour changes. The next step, if taken,
is to generate a CSV for the 956 and measure it against the eight baseline
cells, the compound metric, the spurious-merge gate and the eighteen
sentences — at which point the question stops being "is this the right
mechanism" and becomes "does this list pay".
