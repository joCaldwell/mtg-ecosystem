# Railway deployment configuration

This configuration owns only the `deck-builder` project, scoped to the linked
`production` or `staging` environment. Production follows `release`; staging
follows `main`. Each has one replica, its own 5000 MB `/data` volume and a
backup bucket. Secrets remain sealed platform variables, referenced through
`preserve()`; the production claim code is deliberately absent after claiming.

Use Railway CLI 5.63.1 and install the isolated authoring dependency:

```sh
npm ci --prefix .railway
railway link --project deck-builder --environment staging --service deck-builder
railway config plan
railway link --project deck-builder --environment production --service deck-builder
railway config plan
```

Never apply against another project. Review the plan before `railway config
apply`; an omitted managed resource can be deleted. Do not import plaintext
variables into this file. The initial live setup used scoped CLI/API commands;
this authoring file was checked against the resulting environments. Billing
alerts and native snapshot schedules are outside the authoring SDK graph.

See [the full operations runbook](../packages/deck-builder/OPERATIONS.md) for
build context, promotion, rollback, secret rotation, maintenance and restore,
claim lifecycle, live evidence, monitoring and the Hobby native-backup limit.
