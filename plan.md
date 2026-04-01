Plan:
1. In `query_mask`, we map `gate_spans` (which are `TimeSpan`s) into `Event`s with value 1.0, just to pass them to `compute_event_fragment_boundaries`.
2. Wait, `compute_event_fragment_boundaries` is also called by `apply_event_fragments`, which takes `&[&[Event<f64>]]`.
3. If we change `compute_event_fragment_boundaries` to accept an iterator of `TimeSpan` (or rather `Rational` boundaries) it could avoid creating `gate_events` `Vec<Event>`.
4. Wait, the `query_mask` function in `orpheus-lang/src/value.rs` maps `gate_spans` (a `Vec<TimeSpan>`) into a new `Vec<Event<f64>>` using `.collect::<Vec<_>>()`.
   If I change `compute_event_fragment_boundaries` to take `&[&[TimeSpan]]` or `&[&[Event<f64>]]` it could be tricky.
   But what does `compute_event_fragment_boundaries` actually use? It just uses `.part.start()` and `.part.end()`.
   I can define a trait or simply pass `&[&TimeSpan]` but `control_event_lists` contains `Event<f64>`. So `compute_event_fragment_boundaries` needs a way to extract spans. Or we could just change `query_mask` to not use `compute_event_fragment_boundaries` directly, or define a new helper.

Wait, `compute_event_fragment_boundaries` is used twice.
`apply_event_fragments` passes `control_event_lists` of type `&[&[Event<f64>]]`.
`query_mask` passes `&[&gate_events[..]]` where `gate_events` is a `Vec<Event<f64>>` created just for this.
I can refactor `compute_event_fragment_boundaries` to take an iterator or `impl Iterator<Item = &TimeSpan>`, or simply pass a slice of `TimeSpan` by mapping the `control_event_lists`? Mapping might also collect.

Let's look at `query_mask` in `value.rs`:
```rust
    let gate_events = gate_spans
        .into_iter()
        .map(|part| Event {
            whole: None,
            part,
            value: 1.0,
        })
        .collect::<Vec<_>>();
```
And then it does:
```rust
        let Some(boundaries) = compute_event_fragment_boundaries(&event.part, &[&gate_events[..]])
        else {
            continue;
        };
        for window in boundaries.windows(2) { ... }
```
Instead of converting `gate_spans` into `gate_events`, we could change `compute_event_fragment_boundaries` to accept `impl Iterator<Item = &TimeSpan>`? No, it takes a list of lists.
What if `query_mask` just implements its own boundary gathering?
Or what if `compute_event_fragment_boundaries` takes a single slice `&[TimeSpan]` for `query_mask` and `apply_event_fragments` maps its events to `TimeSpan`? No, wait...

Let's modify `compute_event_fragment_boundaries` to take `&[&[TimeSpan]]` or we can introduce `compute_span_boundaries(source_span, spans: impl Iterator<Item = &TimeSpan>)`.

Let's look at `compute_event_fragment_boundaries`:
```rust
fn compute_event_fragment_boundaries<'a>(
    source_span: &'a TimeSpan,
    control_event_lists: &[&'a [Event<f64>]],
) -> Option<Vec<&'a Rational>> {
...
```
I can change `compute_event_fragment_boundaries` to just take a single slice of events, or maybe just `spans: impl Iterator<Item = &'a TimeSpan>`. Wait, `control_event_lists` is `&[&[Event<f64>]]`.

If I create:
```rust
fn compute_span_boundaries<'a, I>(
    source_span: &'a TimeSpan,
    spans: I,
    capacity_estimate: usize,
) -> Option<Vec<&'a Rational>>
where
    I: Iterator<Item = &'a TimeSpan> + Clone,
{
```
Then `query_mask` could do:
```rust
    let Some(boundaries) = compute_span_boundaries(
        &event.part,
        gate_spans.iter(),
        2 + gate_spans.len() * 2
    )
```
And `apply_event_fragments` could do:
```rust
    let capacity_estimate = 2 + control_event_lists.iter().map(|list| list.len()).sum::<usize>() * 2;
    let spans = control_event_lists.iter().flat_map(|list| list.iter().map(|event| &event.part));
    let Some(boundaries) = compute_span_boundaries(&event.part, spans, capacity_estimate)
```
This entirely eliminates the `gate_events` allocation!

Let's see if there's any other place `gate_events` is used in `query_mask`:
```rust
        for window in boundaries.windows(2) {
            let &[start, end] = window else {
                continue;
            };
            if start >= end {
                continue;
            }

            let part = build_span(start.clone(), end.clone())?;
            if gate_events
                .iter()
                .any(|gate_event| spans_overlap(&gate_event.part, &part))
            {
                masked.push(Event {
                    whole: None,
                    part,
                    value: event.value.clone(),
                });
            }
        }
```
Oh, `gate_events` is also used in `gate_events.iter().any(|gate_event| spans_overlap(&gate_event.part, &part))`.
But `gate_event.part` is just the `TimeSpan`! So `gate_spans.iter().any(|gate_span| spans_overlap(gate_span, &part))` works exactly the same!

This means we don't need `gate_events` AT ALL.
