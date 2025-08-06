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
    expect(res.body.address).to.match(/^zregtestsapling1[0-9a-z]+$/);
  });
});

describe('POST /create_address - exact match', function () {
  this.timeout(5000); // increase timeout if needed

  const accountIndex = 1;
  const expectedResults = [
    {
      address: "zregtestsapling1dkndeymandsp7qx2rc3l5t8u2hlu9wcydtaxmpd6teuysanlmspts54le5qyqsxjwg3fj95a8dn",
      address_index: 1
    },
    {
      address: "zregtestsapling1rlxqfpwfz844jgq0z2fr4jcf4nklmerg9hh2hac9flgzskp680vet7ppe8tkds4m3ncavqpp9w7",
      address_index: 2
    },
    {
      address: "zregtestsapling1duj99qsqx22ax5rvgj6q6q5j5ecfr4h0lkyh3yj98ksnl5plc36824cuwltav3nfxsh62y6n3yz",
      address_index: 3
    },
    {
      address: "zregtestsapling1wa32v4u96vy6lfner6kpuzqpxlmuq90xr2cck3vrtyhfg7huaye58jnc7e9vhg90u70sxay9kts",
      address_index: 4
    },
    {
      address: "zregtestsapling1dle4r23akdxu69g5lfkcqvxu8ec4f9647rvfldnugqjx2hzm5czer2f2e0d3gq3el7k35f4ql3e",
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
