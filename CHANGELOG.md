<a name="0.1.4"></a>
## 0.1.4 (2021-06-07)


#### Bug Fixes

*   only permit Firefox UA from querying contile (#127) ([307b507c](https://github.com/mozilla-services/contile/commit/307b507c964fcabfb78f5268c6f1e152ca5fa3ad))

#### Features

*   redirect root requests to official documentation (#133) ([3b6fa93a](https://github.com/mozilla-services/contile/commit/3b6fa93a525c7938c7ea1ca8cc94c338fe232d8e))
*   Add integration tests (#128) ([fc71c3bd](https://github.com/mozilla-services/contile/commit/fc71c3bd252f63ee97ae84dfac7c037c127d7725))



<a name="0.1.3"></a>
## 0.1.3 (2021-05-28)


#### Features

*   use default location for unlocatable IPs (#115) ([eba159ba](https://github.com/mozilla-services/contile/commit/eba159ba39f97f65e96c21c7f4f0303ba00832e5), closes [#95](https://github.com/mozilla-services/contile/issues/95))
*   don't fail to decode on adM's empty response (#117) ([c5f99231](https://github.com/mozilla-services/contile/commit/c5f992311492e5eefb6beac1cae374dc9bd9d578), closes [#116](https://github.com/mozilla-services/contile/issues/116))



<a name="0.1.2"></a>
## 0.1.2 (2021-05-27)


#### Bug Fixes

*   pin sentry to 0.19 (#114) ([91ebf484](https://github.com/mozilla-services/contile/commit/91ebf48402d227f8e762af7eb0235deecaa52353), closes [#111](https://github.com/mozilla-services/contile/issues/111))
*   advertiser_url -> url in the response (#112) ([66dd479a](https://github.com/mozilla-services/contile/commit/66dd479a86a8ae6548b97bea6ff0e8348b9d9cfa), closes [#110](https://github.com/mozilla-services/contile/issues/110))

#### Features

*   relax click/impression host checking (#113) ([fded42cc](https://github.com/mozilla-services/contile/commit/fded42cc70dd014d065b9d268e26390bbe7a639e), closes [#109](https://github.com/mozilla-services/contile/issues/109))
*   Add improved response codes (#104) ([04b6fa09](https://github.com/mozilla-services/contile/commit/04b6fa09e796b52f5231260a133868134a816f25))
*   Add a setting which defines a custom location header (#102) ([09a75f35](https://github.com/mozilla-services/contile/commit/09a75f35bcc5f66fb7dbb05d8f9accc3b1db7371), closes [#101](https://github.com/mozilla-services/contile/issues/101))



<a name="0.1.1"></a>
## 0.1.1 (2021-05-24)


#### Features

*   minor updates per the latest Tiles API (#97) ([44386c0c](https://github.com/mozilla-services/contile/commit/44386c0cc2c8c1803542764c5d968676947c3c08), closes [#96](https://github.com/mozilla-services/contile/issues/96))



<a name="0.1.0"></a>
## 0.1.0 (2021-05-20)


#### Bug Fixes

*   Add additional verification checks. (#80) ([5e52f244](https://github.com/mozilla-services/contile/commit/5e52f24490cbd64b663c9ead8d23a7bdac3d73ec), closes [#22](https://github.com/mozilla-services/contile/issues/22))
*   quick fixes to support the new adM API (#78) ([80518794](https://github.com/mozilla-services/contile/commit/80518794e3adb61bab616a743eb68db26baf3a65), closes [#77](https://github.com/mozilla-services/contile/issues/77))
*   handle bad responses from ADM (#57) ([352828d1](https://github.com/mozilla-services/contile/commit/352828d1981e7780882fae6624ba5f44fb2d1ddf), closes [#54](https://github.com/mozilla-services/contile/issues/54))
*   propagate $APPNAME through docker stages (#26) ([81f91f8b](https://github.com/mozilla-services/contile/commit/81f91f8bec90e67ec7cbcf34f266c4514516135e), closes [#25](https://github.com/mozilla-services/contile/issues/25))

#### Chore

*   bump rust docker ([d2d86d2a](https://github.com/mozilla-services/contile/commit/d2d86d2a2276f03abcfc8ed194984a490e0759c7))
*   Add documentation (#89) ([b719705d](https://github.com/mozilla-services/contile/commit/b719705d7d6e4716a0c9a4682f415a26c1e2609b))
*   Remove skeleton from README ([a7299ae5](https://github.com/mozilla-services/contile/commit/a7299ae52e66d14c87eb76da69b3929af88b9509))
*   rename fx-tiles -> contile (#14) ([f0f26620](https://github.com/mozilla-services/contile/commit/f0f2662027a512705afb5e8e071bd5d7681158dd), closes [#13](https://github.com/mozilla-services/contile/issues/13))
*   update dependencies to address audit issue (#12) ([04d7e5cf](https://github.com/mozilla-services/contile/commit/04d7e5cfe169b24e21b021f3cddd7d9eae712993))

#### Refactor

*   fix cargo doc warnings ([b7cdb4a0](https://github.com/mozilla-services/contile/commit/b7cdb4a026eec5c92e1f0c370bea5da00fdca3fe))
*   adm.rs -> adm mod part 2 ([f729bef5](https://github.com/mozilla-services/contile/commit/f729bef52c97a22030cf5540cdd8a141d9724a57))
*   adm.rs -> adm mod ([ed94737f](https://github.com/mozilla-services/contile/commit/ed94737f3360706357c3c2c1b9f79a408026df1d))
*   Move Dockerflow behavior into a module (#27) ([d4350af0](https://github.com/mozilla-services/contile/commit/d4350af052276ad954cc49b580698d5398d298f2))

#### Features

*   cleanup the cache key ([509e7133](https://github.com/mozilla-services/contile/commit/509e7133b0e97cf6649dbe98833cb8f4577910ec))
*   Add test GeoCity database (#82) ([e182cc52](https://github.com/mozilla-services/contile/commit/e182cc529d1b6fd0abc1f7722f51b6ecb472171c))
*   Make "country" and "placement" optional args (#84) ([34516071](https://github.com/mozilla-services/contile/commit/34516071132779cd8348edcd5c664f00e17afe48))
*   Add initial filtering (#51) ([8007794c](https://github.com/mozilla-services/contile/commit/8007794c9f10d71325efef17864277c61d6de4e3), closes [#50](https://github.com/mozilla-services/contile/issues/50))
*   add image storage to google buckets (#29) ([6e3b34a5](https://github.com/mozilla-services/contile/commit/6e3b34a5bf96b1cbee8e758bfc3c01d98f3bd8ee))
*   add caching (#16) ([d26215a5](https://github.com/mozilla-services/contile/commit/d26215a5c70d6cd93047606834703ed95fd71d1d), closes [#6](https://github.com/mozilla-services/contile/issues/6))
*   add initial tile proxying (#9) ([71b9df3a](https://github.com/mozilla-services/contile/commit/71b9df3a3eb4b288ddc1344c06bbbfcc66182939), closes [#5](https://github.com/mozilla-services/contile/issues/5))
*   init from mozilla-services/skeleton ([a72c435e](https://github.com/mozilla-services/contile/commit/a72c435ebb4174f71d8b8a48f276a3bee640f3da))
*   initial commit (add MPL 2) ([ebbce6b6](https://github.com/mozilla-services/contile/commit/ebbce6b6f3a611e554de18db958763763e7f4a0a))



