# Asset stub (CI placeholder)

Regenerate with `python tools/make_asset_stub.py`.

This tree exists only so headless CI can exercise texture/audio code paths when the
retail `work/all` directory is absent (DECISIONS D-11: retail assets never enter Git).
Every PNG is an 18x18 solid colour (the retail window skin is 18x18) and `Sound/stub.wav`
is 8 silent 16-bit samples. `AssetDB` switches here only when the **implicit** default
root has no files; explicit roots are never substituted.
