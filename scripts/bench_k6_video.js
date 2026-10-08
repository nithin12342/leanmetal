import http from 'k6/http';
import { check, sleep } from 'k6';

// k6 benchmark script reproducing the exact video social journey:
// 1. GET /feed (Timeline)
// 2. Think time: 3 to 7 seconds (average 5s)
// 3. GET /posts/:id (View post)
// 4. Think time: 3 to 8 seconds (average 5.5s)
// 5. 15% probability: POST /posts/:id/like
// 6. 2% probability: POST /posts (Create post)
// 7. Think time: 5 to 15 seconds (average 10s)
// SLA: P95 latency < 1000ms, Error rate < 1%

const BASE_URL = __ENV.BASE_URL || 'http://social-server:8081';
const TOTAL_POSTS = 500000;
const TOTAL_USERS = 50000;

export const options = {
  discardResponseBodies: true,
  scenarios: {
    social_journey: {
      executor: 'ramping-vus',
      startVUs: 1000,
      stages: [
        { duration: '30s', target: 5000 },
        { duration: '45s', target: 10000 },
        { duration: '45s', target: 15000 },
        { duration: '45s', target: 20000 },
        { duration: '45s', target: 25000 },
        { duration: '30s', target: 25000 },
        { duration: '15s', target: 0 },
      ],
      gracefulRampDown: '5s',
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.01'], // SLA: < 1% error rate
    http_req_duration: ['p(95)<1000'], // SLA: P95 < 1000 ms
  },
};

export default function () {
  const userId = (__VU % TOTAL_USERS) + 1;
  const postId = Math.floor(Math.random() * 1000) + 1; // popular posts 1..1000

  // Step 1: GET /feed
  const feedRes = http.get(`${BASE_URL}/feed`);
  check(feedRes, { 'feed status 200': (r) => r.status === 200 });

  // Think time: 3 to 7 seconds
  sleep(3 + Math.random() * 4);

  // Step 2: GET /posts/:id
  const postRes = http.get(`${BASE_URL}/posts/${postId}`);
  check(postRes, { 'post status 200': (r) => r.status === 200 });

  // Think time: 3 to 8 seconds
  sleep(3 + Math.random() * 5);

  // Step 3a: 15% probability like
  const roll = Math.random();
  if (roll < 0.15) {
    const likeRes = http.post(`${BASE_URL}/posts/${postId}/like`, '', {
      headers: { 'Content-Type': 'application/json' },
    });
    check(likeRes, { 'like status 200': (r) => r.status === 200 });
  }

  // Step 3b: 2% probability create post
  if (roll >= 0.15 && roll < 0.17) {
    const payload = JSON.stringify({
      author_id: userId,
      content: `k6 virtual user ${userId} iteration post`,
    });
    const createRes = http.post(`${BASE_URL}/posts`, payload, {
      headers: { 'Content-Type': 'application/json' },
    });
    check(createRes, { 'create status 201': (r) => r.status === 201 });
  }

  // Think time: 5 to 15 seconds
  sleep(5 + Math.random() * 10);
}
