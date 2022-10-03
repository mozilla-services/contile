<a name="1.8.1"></a>
## 1.8.1 (2022-09-29)

* Retag 1.8.0


<a name="1.8.0"></a>
## 1.8.0 (2022-03-14)


#### Chore

* **deps:**
  *  bump jinja2 from 2.11.2 to 2.11.3 in /smoke-tests/runner (#366) ([57d1ce09](https://github.com/mozilla-services/contile/commit/57d1ce0948b573f40134d9da93d9303eb2fa197b))
  *  bump jinja2 from 2.11.2 to 2.11.3 in /smoke-tests/client (#365) ([16789acc](https://github.com/mozilla-services/contile/commit/16789acc70f9a91d795228d06bfd554a319c2fd0))

#### Features

*   include os-family and form-factor as metric tags (#371) ([25a551bf](https://github.com/mozilla-services/contile/commit/25a551bfffe04def158b1a9b06ac0acc71f6aea4), closes [#369](https://github.com/mozilla-services/contile/issues/369))

#### Test

*   add Cloud Function based geo smoke-tests (#364) ([11203995](https://github.com/mozilla-services/contile/commit/112039955818219891ecb1511cfbdbb49948578c))



<a name="1.7.0"></a>
## 1.7.0 (2022-02-25)


#### Chore

*   cargo update to resolve RUSTSEC-2022-0006 (#352) ([0c245dda](https://github.com/mozilla-services/contile/commit/0c245ddaf1ebe46987ee0b93873831fdecfb64ba))

#### Bug Fixes

*   add timeouts for reqwest calls (#355) ([ac91d920](https://github.com/mozilla-services/contile/commit/ac91d920c3886845161d42a0eec12c7e8f5d4059))
*   report error from image library (#349) ([8d13e883](https://github.com/mozilla-services/contile/commit/8d13e883f4f2e8cd7153922f82bd0a5b694d522b))

#### Features

*   add a simple image cache to reduce image fetches (#359) ([6c4928e9](https://github.com/mozilla-services/contile/commit/6c4928e930b61f143efe302cf43f2357f72230e6), closes [#357](https://github.com/mozilla-services/contile/issues/357))
*   Add support for multiple ADM endpoints (#346) ([80d7dca4](https://github.com/mozilla-services/contile/commit/80d7dca40f2e8851efa74dab68436f7ed313099a))
*   move cleanup of temp cache states into a ScopeGuard (#351) ([9122ec04](https://github.com/mozilla-services/contile/commit/9122ec04d0afa7a8f424c378ebacda4ac9a99c95), closes [#342](https://github.com/mozilla-services/contile/issues/342))
*   Return JSON error messages (#353) ([fe2c325c](https://github.com/mozilla-services/contile/commit/fe2c325c627ceb8bec4e4eff8ef40a1065372743), closes [#177](https://github.com/mozilla-services/contile/issues/177))
*   Make the partner timeouts "softer" during initial cache loading. (#337) ([99cacad4](https://github.com/mozilla-services/contile/commit/99cacad4e7357b2a84a0daf4965ecddc2050e4eb), closes [#336](https://github.com/mozilla-services/contile/issues/336))
*   Read ADM settings data from a Google Storage bucket. (#331) ([5e85c8fe](https://github.com/mozilla-services/contile/commit/5e85c8fe424dd0c57c815bdb1decccec625bfcdf), closes [#324](https://github.com/mozilla-services/contile/issues/324))

#### Test

*   upgrade contile-integration-tests Docker images 🤖 (#350) ([281c2864](https://github.com/mozilla-services/contile/commit/281c286417a9abd2948b8db1665fab02baa9c640))



<a name="1.6.0"></a>
## 1.6.0 (2021-12-08)


#### Bug Fixes

*   fix YAML schema warning for CircleCI config (#329) ([6f9afc3e](https://github.com/mozilla-services/contile/commit/6f9afc3e2a7ee463040380c824d9dece11a8d09b))

#### Chore

*   2021 Q4 dependency update (#321) ([de6ae1e2](https://github.com/mozilla-services/contile/commit/de6ae1e24a702c29575a294d90429c2a863b290a), closes [#320](https://github.com/mozilla-services/contile/issues/320))

#### Features

*   only include hostname in InvalidHost's err message (#327) ([77278272](https://github.com/mozilla-services/contile/commit/77278272edaee2cd9e0470720716e3105927a290), closes [#322](https://github.com/mozilla-services/contile/issues/322), [#323](https://github.com/mozilla-services/contile/issues/323))



<a name="1.5.1"></a>
## 1.5.1 (2021-11-10)


#### Test

*   add integration test coverage for path filtering (#313) ([bc89e5f2](https://github.com/mozilla-services/contile/commit/bc89e5f206e7ee7301e17f7252c6d56bdeef3125))

#### Bug Fixes

*   ensure settings from file are filtered like env settings (#314) ([b510434d](https://github.com/mozilla-services/contile/commit/b510434d6deccfd75fce11560d67cdb0c7860e4f))

#### Features

*   Refine the host & pathing filter (#311) ([99ee20ea](https://github.com/mozilla-services/contile/commit/99ee20ea8c926b57f656201ae3b354c36259e001))



<a name="1.5.0"></a>
## 1.5.0 (2021-11-03)


#### Refactor

*   Simplify various converters for tags (#307) ([6dc80d0e](https://github.com/mozilla-services/contile/commit/6dc80d0edd043d4e10fd64af60b0aeeff4c501a5))
*   reorganize integration-tests files (#297) ([23bdb2fd](https://github.com/mozilla-services/contile/commit/23bdb2fd6965c479a226ea906612567e2dbac052))

#### Features

*   add rule parser for advertiser hosts (#304) ([0a287229](https://github.com/mozilla-services/contile/commit/0a2872299e788da6001869ffb2986745cf011b8c), closes [#303](https://github.com/mozilla-services/contile/issues/303))
*   don't emit InvalidUA to sentry, incr a metric instead (#306) ([dc8e94a5](https://github.com/mozilla-services/contile/commit/dc8e94a53412c149a78500ad7a2bfec00f6ee647), closes [#275](https://github.com/mozilla-services/contile/issues/275))
*   Add local server hostname to tags (#234) ([ba04b245](https://github.com/mozilla-services/contile/commit/ba04b24502a84c989adde87374f93c0137ec6d90), closes [#200](https://github.com/mozilla-services/contile/issues/200))

#### Chore

*   Add templates for Pull Requests (#278) ([c761902e](https://github.com/mozilla-services/contile/commit/c761902ee702db36eba6293dfa7949ed21e2de27))
*   delete unused docker-compose example file (#296) ([5f86beb7](https://github.com/mozilla-services/contile/commit/5f86beb7f074fcc765d42d935a91a8c293556223))

#### Test

*   upgrade the Docker images for contile-integration-tests to 21.6.0 (#295) ([cbe464b7](https://github.com/mozilla-services/contile/commit/cbe464b79df90677922ea6f62b1cfec7e53f60fc))
*   add integration tests for 200 OK for excluded countries (#294) ([b11cc74e](https://github.com/mozilla-services/contile/commit/b11cc74e3db1c4c72f6a318d3faf3887b7e0108b))



<a name="1.4.0"></a>
## 1.4.0 (2021-09-23)


#### Features

*   add a `/__test_loc__` dockerflow endpoint (#282) ([d28584d9](https://github.com/mozilla-services/contile/commit/d28584d94338ead8a56cb04cf15307ed1f6ee4c4), closes [#281](https://github.com/mozilla-services/contile/issues/281))
*   optionally send 200 OK empty responses to excluded countries (#290) ([bf8594e0](https://github.com/mozilla-services/contile/commit/bf8594e03ed8d9acc71814db0c322e97d86ce205), closes [#284](https://github.com/mozilla-services/contile/issues/284))



<a name="1.3.0"></a>
## 1.3.0 (2021-08-24)


#### Test

*   upgrade the Docker images for contile-integration-tests to 21.4.0 (#262) ([de43bf16](https://github.com/mozilla-services/contile/commit/de43bf16c695c54d6fbb69bcfde1f1746c84812c))

#### Chore

*   API Updates for 2021-08-19 (#272) ([84b79088](https://github.com/mozilla-services/contile/commit/84b790884f3880d562489b14c9ca47eb51c26d66), closes [#268](https://github.com/mozilla-services/contile/issues/268))

#### Features

*   set a connect_timeout for reqwest client ([6a363f21](https://github.com/mozilla-services/contile/commit/6a363f212827d167692c10d326a96a237219de2c), closes [#253](https://github.com/mozilla-services/contile/issues/253))
*   don't hide cloud storage update errors ([5fad141a](https://github.com/mozilla-services/contile/commit/5fad141afd1a55ebd0b68bc2f51a9c55bddc939c), closes [#263](https://github.com/mozilla-services/contile/issues/263))
*   support dma-codes in location_test_header (#271) ([74e053ec](https://github.com/mozilla-services/contile/commit/74e053ec0e2d1de54aeb1432842f1fba135c1ecc), closes [#269](https://github.com/mozilla-services/contile/issues/269))
*   enable send of dma-code (#235) ([5da0bda0](https://github.com/mozilla-services/contile/commit/5da0bda0e3539336e98f3e479e18b28153f44004), closes [#205](https://github.com/mozilla-services/contile/issues/205))
*   add a cloud storage write precondition (#260) ([d0bddfd6](https://github.com/mozilla-services/contile/commit/d0bddfd63179bd1829007a77305195560d2afc34), closes [#259](https://github.com/mozilla-services/contile/issues/259))

#### Bug Fixes

*   re-add location_test_header support (#264) ([4c6c2cad](https://github.com/mozilla-services/contile/commit/4c6c2cadaa90f01bd067dab8009444c3352af98c), closes [#257](https://github.com/mozilla-services/contile/issues/257))

#### Refactor

*   Replace location determination with common-rs crate (#219) ([ac2783ca](https://github.com/mozilla-services/contile/commit/ac2783ca34c6763ac7d60816874fd046d8aab2f3))



<a name="1.2.1"></a>
## 1.2.1 (2021-08-10)


#### Test

*   Re-enable automated integration tests (#255) ([fcae97d7](https://github.com/mozilla-services/contile/commit/fcae97d7d9e721994b997d89d8c0b72a906c7272))



<a name="1.2.0"></a>
## 1.2.0 (2021-08-10)


#### Chore

*   cleanup bucket creation (#245) ([2a7bc7bb](https://github.com/mozilla-services/contile/commit/2a7bc7bb073636a8c3e5eca31bb983f5a3ba010a), closes [#245](https://github.com/mozilla-services/contile/issues/245))

#### Features

*   reduce redundant adM requests (#250) ([9e98c998](https://github.com/mozilla-services/contile/commit/9e98c998ec03dbf53e7fa5bce24905358cd8ef9d), closes [#248](https://github.com/mozilla-services/contile/issues/248))
*   Add `adm_has_legacy_image` setting to filter <v91 tiles (#247) ([b87e9b4f](https://github.com/mozilla-services/contile/commit/b87e9b4f31b97e2049db7defb81745f0cefc10e0), closes [#246](https://github.com/mozilla-services/contile/issues/246))

#### Test

*   upgrade the Docker images for contile-integration-tests (#241) ([1c3ef82f](https://github.com/mozilla-services/contile/commit/1c3ef82f2def629fe9eedb1cfef5dd34b239dfc0))



<a name="1.1.1"></a>
## 1.1.1 (2021-08-05)


#### Features

*   don't create the cloud storage bucket by default (#244) ([b25c995e](https://github.com/mozilla-services/contile/commit/b25c995e72989b5cf8627fa195a019a542e0e259), closes [#243](https://github.com/mozilla-services/contile/issues/243))



<a name="1.1.0"></a>
## 1.1.0 (2021-08-05)


#### Bug Fixes

*   Write version.json before docker build ([32818038](https://github.com/mozilla-services/contile/commit/32818038757b2dcf90a73c1858fed31e33299cb3))
*   Allow '-' in bucket name ([ce70f9fc](https://github.com/mozilla-services/contile/commit/ce70f9fce54f91d016bdbc82613dab91c34a8f1d))

#### Test

*   block deploy CI job on integration-tests (#239) ([3fed603d](https://github.com/mozilla-services/contile/commit/3fed603d7a598745619a5a083e876041ec66f1ea), closes [#238](https://github.com/mozilla-services/contile/issues/238))

#### Features

*   optimize handling of not included countries (#240) ([76b0fe02](https://github.com/mozilla-services/contile/commit/76b0fe022fdf7081d48ff882c213fd473007d901), closes [#226](https://github.com/mozilla-services/contile/issues/226))



<a name="1.0.0"></a>
## 1.0.0 (2021-08-02)


#### Features

*   Switch hasher to blake3 (#229) ([aab13283](https://github.com/mozilla-services/contile/commit/aab132833e8416662f8039eb4cfcd5e28ca697d4), closes [#228](https://github.com/mozilla-services/contile/issues/228))
*   record metric for empty ADM responses (#223) ([13ee0874](https://github.com/mozilla-services/contile/commit/13ee08745487a5fa53b2a64e7042c1b6dd501cf6), closes [#222](https://github.com/mozilla-services/contile/issues/222))
*   Send image URLs to CDN. (#212) ([1e3c08c0](https://github.com/mozilla-services/contile/commit/1e3c08c007ed68fb8f860156b84346bc19bb0bd7), closes [#167](https://github.com/mozilla-services/contile/issues/167))

#### Chore

*   Update code for newest rust 1.54 (#231) ([f063e818](https://github.com/mozilla-services/contile/commit/f063e8183731b2e9ec60dc6e619aa34afec9611b))



<a name="0.4.1"></a>
## 0.4.1 (2021-07-21)


#### Chore

*   Security checklist final bits (#211) ([132ec029](https://github.com/mozilla-services/contile/commit/132ec029cadccc10382aafebde541c94543405fe))

#### Bug Fixes

*   take include_regions into account for filtering  (#217) ([14d38926](https://github.com/mozilla-services/contile/commit/14d3892633d0e99021fe8eaa742ce82fa17c4547), closes [#216](https://github.com/mozilla-services/contile/issues/216))

#### Test

* **Integration_test:**  add integration test to the CircleCI workflow (#214) ([bc8d5f29](https://github.com/mozilla-services/contile/commit/bc8d5f29e406f57cfbbae533ee5cbd21cd49e38e))



<a name="0.4.0"></a>
## 0.4.0 (2021-06-25)


#### Features

*   optionally include location info in /__error__ (w/ ?with_location=true) (#198) ([f0be5e9d](https://github.com/mozilla-services/contile/commit/f0be5e9d7c6b0fabe0cfe723e6f0f314bfb573d4), closes [#192](https://github.com/mozilla-services/contile/issues/192))

#### Refactor

*   kill the old adM API's country mapping ([11ff5ece](https://github.com/mozilla-services/contile/commit/11ff5ece683f997e11c6f29d49b5c2ff509e55c6), closes [#195](https://github.com/mozilla-services/contile/issues/195))
*   kill unused ua code ([26f10ce5](https://github.com/mozilla-services/contile/commit/26f10ce52fbd898afc1c4eb60dc004f6537b8e92))

#### Bug Fixes

*   fix maxmind country/subdivision to use iso_code (#193) ([bdfdf24d](https://github.com/mozilla-services/contile/commit/bdfdf24d2c50fd12eb7ae3d3cf230dcd771c3c6a), closes [#183](https://github.com/mozilla-services/contile/issues/183), [#184](https://github.com/mozilla-services/contile/issues/184))
*   return 204 no contents for cache hits (#199) ([b8afa238](https://github.com/mozilla-services/contile/commit/b8afa23808203178922c9d2f2dc84882df784cf3), closes [#191](https://github.com/mozilla-services/contile/issues/191))



<a name="0.3.0"></a>
## 0.3.0 (2021-06-17)


#### Features

*   fill in sentry stacktraces (#159) ([623028fd](https://github.com/mozilla-services/contile/commit/623028fd882e68231e171215c395577c5f77d85f), closes [#158](https://github.com/mozilla-services/contile/issues/158))
*   metric calls to adM (#157) ([5edcca92](https://github.com/mozilla-services/contile/commit/5edcca92f53541e43d4bec4eef271459f9cde35a), closes [#138](https://github.com/mozilla-services/contile/issues/138))
*   don't fallback on unknown subdivisions (#156) ([5d02edff](https://github.com/mozilla-services/contile/commit/5d02edff03e513ea64e1433e7075642e96bfdbc8), closes [#148](https://github.com/mozilla-services/contile/issues/148))
*   get the client IP for mmdb from X-Forwarded-For (#155) ([2a3882de](https://github.com/mozilla-services/contile/commit/2a3882de79f1b91d55ad0d910b69ee2aeea480dc))
*   include ip addr for diagnosing mmdb lookup failures (#154) ([4f1d61f8](https://github.com/mozilla-services/contile/commit/4f1d61f8c9ac26382651983649905c6402cd52c8))
*   add an optional trace header to metrics/logging (#146) ([762bc398](https://github.com/mozilla-services/contile/commit/762bc398aa82d9d987cf140ebce7fc75a1a55091), closes [#145](https://github.com/mozilla-services/contile/issues/145))
*   integrate cache updating into the GET request (#152) ([109ec114](https://github.com/mozilla-services/contile/commit/109ec114e4f4780ddbf55dc9dc4b6060ea254eb9), closes [#151](https://github.com/mozilla-services/contile/issues/151))

#### Bug Fixes

*   make UA name an 'extra' value for Sentry errors. (#153) ([582c3270](https://github.com/mozilla-services/contile/commit/582c3270b6c526d2345ad1c538e2b5d6a69aab1d), closes [#147](https://github.com/mozilla-services/contile/issues/147))



<a name="0.2.0"></a>
## 0.2.0 (2021-06-08)


#### Features

*   Remove useless metric tags (#142) ([6ea90cde](https://github.com/mozilla-services/contile/commit/6ea90cde13049a3f32f871f3f9028d546e6b7945), closes [#136](https://github.com/mozilla-services/contile/issues/136))
*   add timeout for calls to ADM service (#140) ([4837bb51](https://github.com/mozilla-services/contile/commit/4837bb514d98acf8b294cffb28a93452228a89c3), closes [#139](https://github.com/mozilla-services/contile/issues/139))
*   add more metrics (#130) ([786fe729](https://github.com/mozilla-services/contile/commit/786fe729fecf2d1e1d9d72b1d719270831dc3134), closes [#120](https://github.com/mozilla-services/contile/issues/120))

#### Chore

*   tag 0.1.4 (#141) ([bc897fe4](https://github.com/mozilla-services/contile/commit/bc897fe4b5270565bc6d6580c058c287373ce40f))



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



