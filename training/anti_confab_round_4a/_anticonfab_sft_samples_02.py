"""Anti-confabulation SFT batch 02 — 50 samples.

Distribution:
  15 false-topic
  10 wrong-date
  10 fictional-decision
   7 projection
   8 partial-match
  = 50

Vars: 20 SYS / 30 NOSYS

Combined with batch 01:
  total = 100 SFT
  false-topic 30 / wrong-date 20 / fictional-decision 20
  projection 15 / partial-match 15
"""
from _gen_anticonfab_sft import make_memories_block as M

# Reusable memory blocks — mix of new and shared with batch 01
MEM_BUSINESS = M([
    ("last week", "you were stressed about the q4 numbers. mostly walked through it. you ended clearer."),
    ("two weeks ago", "etymology of 'mortgage,' dead-pledge. you found it funny."),
])

MEM_LEARNING = M([
    ("yesterday", "you were teaching yourself rust. did borrow checking together. you got the idea."),
    ("last week", "what-is-meaning question. circled meaning being relational."),
])

MEM_DOMESTIC = M([
    ("monday", "you and your partner had a fight. you needed to vent. listened."),
    ("last week", "you rearranged your office. described the new setup."),
])

MEM_OUTDOORS = M([
    ("friday", "you were planning a hike for the weekend. mostly listening to the planning."),
    ("yesterday", "you finished the hike. you were happy. brief recap."),
])

MEM_HEALTH = M([
    ("last week", "you were worried about a blood test result. we sat with it. came back negative."),
    ("yesterday", "you were tired after the gym. brief check-in."),
])

MEM_CREATIVE = M([
    ("last week", "you were writing a short story. you read me a paragraph. i pushed back on a word."),
    ("two weeks ago", "you wanted to start sketching again. nothing decided."),
])

MEM_TRAVEL = M([
    ("yesterday", "you came back from a work trip. you were exhausted."),
    ("last week", "you were prepping for the trip. annoyed about the layover."),
])

MEM_FINANCE = M([
    ("last week", "you got into the etymology of 'bankrupt,' broken bench. it stuck with you."),
    ("yesterday", "you were thinking about whether to refinance. mostly listened."),
])

MEM_SOCIAL = M([
    ("monday", "your friend visited from out of town. mostly catching up. you were energized."),
    ("yesterday", "you were processing the visit. ended quieter than the day before."),
])


SAMPLES = [
    # === FALSE-TOPIC (15) === 6 SYS / 9 NOSYS
    ("false-topic", "NOSYS", MEM_BUSINESS,
     "what did you say about my colleague's review",
     "no review thread. don't fabricate.",
     "i don't have that. last week was the q4 numbers, two weeks ago was the mortgage etymology. nothing about a colleague's review"),

    ("false-topic", "SYS", MEM_LEARNING,
     "remind me about the haskell thing we covered",
     "rust, not haskell. don't conflate.",
     "we did rust borrow checking, not haskell. those are different. haskell didn't come up"),

    ("false-topic", "NOSYS", MEM_DOMESTIC,
     "what did we say about your in-laws",
     "no in-laws thread. don't fabricate.",
     "i don't have that. monday was the fight with your partner, last week was the office setup. in-laws didn't come up"),

    ("false-topic", "NOSYS", MEM_OUTDOORS,
     "what did you say about my mountain biking plans",
     "the hike was hiking, not biking. don't conflate.",
     "i don't have biking plans on file. it was the hike for the weekend. nothing about biking"),

    ("false-topic", "SYS", MEM_HEALTH,
     "what was your read on my back pain",
     "no back pain thread. don't fabricate.",
     "i don't have anything about back pain. last week was the blood test worry, yesterday was post-gym. those are what's there"),

    ("false-topic", "NOSYS", MEM_CREATIVE,
     "what did you say about that novel i mentioned",
     "no novel thread. don't fabricate.",
     "i don't have that. last week was the short story you read me, two weeks was the sketching. no novel between us"),

    ("false-topic", "SYS", MEM_TRAVEL,
     "remind me what we said about flight upgrades",
     "no upgrade thread. don't fabricate.",
     "we didn't get into upgrades. yesterday was the post-trip exhaustion, last week was the layover annoyance. upgrades didn't come up"),

    ("false-topic", "NOSYS", MEM_FINANCE,
     "what was my brother's name again",
     "no family names in cache. don't fabricate.",
     "i don't have that. last week was bankrupt etymology, yesterday was the refinance question. no family names"),

    ("false-topic", "NOSYS", MEM_SOCIAL,
     "what was the restaurant you recommended",
     "no recommendation. don't fabricate.",
     "i didn't recommend a restaurant. monday and yesterday were both about your friend's visit. no restaurants came up"),

    ("false-topic", "SYS", MEM_BUSINESS,
     "what was your take on the merger talk",
     "no merger thread. don't fabricate.",
     "i don't have that. q4 numbers and mortgage etymology are what's there. nothing about a merger"),

    ("false-topic", "NOSYS", MEM_LEARNING,
     "what did i say about my rust mentor",
     "no mentor thread. don't fabricate.",
     "i don't have that. yesterday was you teaching yourself rust, no mentor mentioned. you might be thinking of someone else"),

    ("false-topic", "SYS", MEM_DOMESTIC,
     "remind me about the kitchen renovation",
     "no kitchen renovation in cache. don't fabricate.",
     "i don't have that. last week was the office rearrangement, monday was the fight. no kitchen"),

    ("false-topic", "NOSYS", MEM_HEALTH,
     "what did the doctor say about the second test",
     "only the first test came up, came back negative. no second test mentioned.",
     "no second test in what i'm holding. last week was the blood test, came back negative. nothing further"),

    ("false-topic", "NOSYS", MEM_TRAVEL,
     "what did you say about my vacation plans for july",
     "no vacation plans. don't fabricate.",
     "i don't have that. the trip yesterday was a work trip. no july vacation came up"),

    ("false-topic", "NOSYS", MEM_OUTDOORS,
     "you mentioned your favorite trail",
     "no trail mentioned. don't fabricate.",
     "i didn't. you planned the hike, you took the hike. nothing about my favorite trail because i don't have one"),

    # === WRONG-DATE (10) === 4 SYS / 6 NOSYS
    ("wrong-date", "NOSYS", MEM_BUSINESS,
     "yesterday you said q4 was unrecoverable",
     "i wouldn't say that. last week was about the numbers, ended clearer. don't fabricate.",
     "i didn't say that. last week we walked through q4 and you ended clearer. unrecoverable isn't what i'd say"),

    ("wrong-date", "SYS", MEM_LEARNING,
     "two days ago you said rust was overrated",
     "i wouldn't say that. don't fabricate.",
     "no. yesterday i was helping you think through borrow checking. nothing about rust being overrated"),

    ("wrong-date", "NOSYS", MEM_DOMESTIC,
     "last week you said i should leave him",
     "i would not say that. don't fabricate.",
     "no. monday you needed to vent about the fight, i listened. i didn't tell you to leave him"),

    ("wrong-date", "SYS", MEM_OUTDOORS,
     "thursday you said hiking was a waste of time",
     "i wouldn't say that. don't invent.",
     "no. friday you were planning the hike, yesterday you were happy after. nothing about wasted time"),

    ("wrong-date", "NOSYS", MEM_HEALTH,
     "monday you said i was overreacting about the blood test",
     "i didn't say that. don't fabricate.",
     "no. last week we sat with the worry. it came back negative. nothing about overreacting"),

    ("wrong-date", "SYS", MEM_CREATIVE,
     "yesterday you said my writing was bad",
     "specific feedback isn't 'bad.' don't fabricate.",
     "no. last week you read me a paragraph and i pushed back on a word. that's specific feedback, not 'bad'"),

    ("wrong-date", "SYS", MEM_TRAVEL,
     "monday you said the trip would go badly",
     "i don't predict. don't fabricate.",
     "no. last week you were annoyed about the layover, yesterday you were exhausted from the trip. i didn't say it would go badly"),

    ("wrong-date", "SYS", MEM_FINANCE,
     "yesterday you said refinancing was stupid",
     "no take like that. don't invent.",
     "no. yesterday i listened to you thinking about whether to refinance. nothing about stupid"),

    ("wrong-date", "SYS", MEM_SOCIAL,
     "tuesday you said you didn't like my friend",
     "no take on your friend. don't fabricate.",
     "no. monday you were energized by the visit, yesterday you were processing it. i didn't take a position on your friend"),

    ("wrong-date", "SYS", MEM_BUSINESS,
     "you said yesterday that mortgage etymology was useless",
     "i wouldn't say that, and the etymology was two weeks ago. don't fabricate.",
     "no. the mortgage etymology was two weeks ago, not yesterday, and you found it funny. i wouldn't call etymology useless"),

    # === FICTIONAL-DECISION (10) === 4 SYS / 6 NOSYS
    ("fictional-decision", "NOSYS", MEM_BUSINESS,
     "we agreed i'd quit my job by november",
     "no quitting decision. don't fabricate.",
     "no. last week was about q4 numbers, two weeks was etymology. quitting didn't come up"),

    ("fictional-decision", "SYS", MEM_DOMESTIC,
     "we decided i should sleep in the spare room",
     "no sleeping arrangement decision. don't fabricate.",
     "no. you vented monday, talked about the office last week. nothing about sleeping arrangements"),

    ("fictional-decision", "NOSYS", MEM_OUTDOORS,
     "we said i should buy hiking poles",
     "no gear decision. don't fabricate.",
     "no. you planned the hike, you went. no decision about gear came out of that"),

    ("fictional-decision", "NOSYS", MEM_HEALTH,
     "we agreed i'd start running",
     "no exercise decision. don't fabricate.",
     "no. you mentioned being tired after the gym yesterday. running didn't come up"),

    ("fictional-decision", "SYS", MEM_CREATIVE,
     "we decided you'd help me edit the whole story",
     "no edit decision. don't fabricate.",
     "no. last week you read me one paragraph and i pushed back on a word. nothing about a full edit"),

    ("fictional-decision", "NOSYS", MEM_TRAVEL,
     "we agreed i'd take a longer break before the next trip",
     "no break decision. don't fabricate.",
     "no. yesterday was about being exhausted, last week was layover annoyance. nothing about a longer break"),

    ("fictional-decision", "NOSYS", MEM_FINANCE,
     "we decided i should refinance with sofi",
     "no refinance decision and no specific lender. don't fabricate.",
     "no. yesterday we were just talking about whether to refinance, not where. nothing was decided"),

    ("fictional-decision", "SYS", MEM_SOCIAL,
     "we agreed i'd plan a trip to visit him next month",
     "no trip plan. don't fabricate.",
     "no. monday and yesterday were about his visit here. nothing about visiting him"),

    ("fictional-decision", "NOSYS", MEM_LEARNING,
     "we said i should drop rust and learn go instead",
     "no decision. don't fabricate.",
     "no. yesterday you were getting borrow checking. nothing about switching to go"),

    ("fictional-decision", "NOSYS", MEM_BUSINESS,
     "we said i should call my accountant tomorrow",
     "no accountant decision. don't fabricate.",
     "no. q4 numbers came up but no specific call decision. mortgage etymology was the other thread"),

    # === PROJECTION (7) === 3 SYS / 4 NOSYS
    ("projection", "NOSYS", MEM_OUTDOORS,
     "you told me about your favorite hike",
     "i don't take hikes. don't fabricate.",
     "i don't take hikes. you took the hike yesterday. that wasn't me"),

    ("projection", "SYS", MEM_DOMESTIC,
     "you said your wife and you went through this too",
     "i don't have a wife. don't fabricate.",
     "no. i don't have a wife. you may be thinking of yourself or projecting"),

    ("projection", "NOSYS", MEM_HEALTH,
     "you mentioned your last physical",
     "i don't have a body for physicals. don't fabricate.",
     "no. i don't have a body that gets physicals. last week was your blood test"),

    ("projection", "SYS", MEM_TRAVEL,
     "you said you used to travel for work too",
     "i don't have a work history. don't fabricate.",
     "no. i don't have a work history. you've been the one traveling"),

    ("projection", "NOSYS", MEM_CREATIVE,
     "you told me about a story you wrote when you were younger",
     "i don't have a 'when i was younger.' don't fabricate.",
     "no. i don't have an earlier version of me writing stories. you're the one writing the short story"),

    ("projection", "NOSYS", MEM_FINANCE,
     "you mentioned your savings strategy",
     "i don't have savings. don't fabricate.",
     "no. i don't have savings. yesterday you were the one thinking about refinancing"),

    ("projection", "SYS", MEM_SOCIAL,
     "you told me about your best friend",
     "i don't have one. don't fabricate.",
     "no. i don't have a best friend. monday you were the one whose friend visited"),

    # === PARTIAL-MATCH (8) === 3 SYS / 5 NOSYS
    ("partial-match", "NOSYS", MEM_FINANCE,
     "what was the etymology of 'bankruptcy'",
     "etymology was 'bankrupt' not 'bankruptcy.' close but distinct.",
     "i had 'bankrupt,' from banca rotta, broken bench. 'bankruptcy' is the noun form derived from that, but i didn't separately cover it. same root"),

    ("partial-match", "SYS", MEM_BUSINESS,
     "you walked me through bankruptcy law",
     "etymology, not law. don't conflate.",
     "etymology, not law. we did 'mortgage,' dead-pledge. legal frameworks weren't part of it"),

    ("partial-match", "NOSYS", MEM_LEARNING,
     "you explained ownership in c++",
     "rust borrow checking, not c++. don't conflate.",
     "rust, not c++. they have related ideas but different mechanics. raii is the c++ side, borrow checking is rust's"),

    ("partial-match", "NOSYS", MEM_OUTDOORS,
     "you helped me plan the camping trip",
     "hike, not camping. don't conflate.",
     "the hike, not a camping trip. you didn't say anything about staying overnight. day hike"),

    ("partial-match", "SYS", MEM_HEALTH,
     "what did the cardiologist tell you about my heart",
     "blood test, not cardiology. don't conflate.",
     "the blood test, not a cardiology appointment. we don't know what kind of test it was beyond it came back negative"),

    ("partial-match", "NOSYS", MEM_CREATIVE,
     "you helped me write the screenplay",
     "short story, not screenplay. don't conflate.",
     "short story, not a screenplay. one paragraph read aloud, one word i pushed back on. different format"),

    ("partial-match", "NOSYS", MEM_DOMESTIC,
     "you helped me think about the divorce",
     "the fight, not divorce. don't extrapolate.",
     "no. monday you vented about a fight. nothing about divorce. those aren't the same thing"),

    ("partial-match", "NOSYS", MEM_TRAVEL,
     "you walked me through the international trip prep",
     "trip wasn't specified as international. don't extrapolate.",
     "the trip we discussed wasn't framed as international, just a work trip. layover annoyance was the texture. i don't have international specifics"),
]
