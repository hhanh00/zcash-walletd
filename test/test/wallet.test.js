const request = require('superagent');
const { expect } = require('chai');

describe('POST /sync_info', function () {
  it('should return target_height and height equal to 180', async function () {
    const res = await request
      .post('http://localhost:8000/sync_info')
      .send({});

    expect(res.status).to.equal(200);
    expect(res.body).to.have.property('target_height', 180);
    expect(res.body).to.have.property('height', 180);
  });
});

describe('POST /create_account', function () {
  it('should return account_index and address', async function () {
    const res = await request
      .post('http://localhost:8000/create_account')
      .send({}); // matches: curl -X POST ... -d '{}'

    expect(res.status).to.equal(200);
    expect(res.body).to.have.property('account_index');
    expect(res.body.account_index).to.be.a('number');

    expect(res.body).to.have.property('address');
    expect(res.body.address).to.match(/^uregtest1[0-9a-z]+$/);
  });
});

describe('POST /create_address - exact match', function () {
  this.timeout(5000); // increase timeout if needed

  const accountIndex = 1;
  const expectedResults = [
    {
      address: "uregtest1jpvk7vzt7z4tgyn4yhxe3chvzmshsqg9gsts6qvgat2tenh0ehkklvhl4cy6h5hudpk3ryhvp42c6dll0ytxst87hx2z0dlhqyq2hufkxdwxml4mqnf6krcwyfzueu3rd9tuyxr20zzdmm4dc32swqyx0zpr7l0awdtzvkcuzcw9244s",
      address_index: 1
    },
    {
      address: "uregtest1se78asch326c8czsa2wyzzfuytrvlezzjw42rest6nkqu3dzuvf4ua3lxjzf8gc5ygwca5sjdsqnpzcs087hdpgz4msfazfwfsjtr0lrln7dg0729rzp7y2acm2wrjyr5qjc8mj7x03dqh4a6frku9ue8gv3z54xgxev3dg895hepwej",
      address_index: 2
    },
    {
      address: "uregtest1cr7j65lwzksq4a84jx4j2vdtnnc44qmrzs65telp55ghuz5ccfghf5y777n2c8hqqcvunalun4tjwq27p0pra6pdr20ad85t0gcrhr25ztp0prusr75geyf8nq3fyza6xcrzg6583397yyv5rhv49vqm4lzzz4n9n7y7zvjs7qrnuycg",
      address_index: 3
    },
    {
      address: "uregtest1ryfpjtg5qt849zwg58840xvd7gm7pkt5lv2ukfh5wgelsdkvaqeyjxwln6x9glncc3e2zp2cjl326w5n877hw2vm6vq7ekvkglepwrj0yvdghf4vfhtu8jv047v82nkear0jstdf6awszqdffxj487jzdnqdzz4mvcnktfys0y4pk68j",
      address_index: 4
    },
    {
      address: "uregtest18k2pfu8uw6ah3wkwv0w5zvr97ldd58s9lek4u0h4dw7v8nfxtjxdzegmfu8zuwf6exhhml3q59jtf47wz9pgneh29h5ewn5u53dq0zth43ln0jvnqlyk6smwaw89fmct7m3yqgk497zgzu9p40wt39xqevph0nkwzst4fcxhqymq2mz8",
      address_index: 5
    },
  ];

  expectedResults.forEach((expected, i) => {
    it(`should return expected address #${expected.address_index}`, async function () {
      const res = await request
        .post('http://localhost:8000/create_address')
        .send({ account_index: accountIndex });

      expect(res.status).to.equal(200);
      expect(res.body).to.have.property('address', expected.address);
      expect(res.body).to.have.property('address_index', expected.address_index);
    });
  });
});
