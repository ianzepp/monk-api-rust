# Public Handlers

**Security Tier 1: No Authentication Required**

Public endpoints that do not require authentication. These are primarily used for token acquisition and public documentation.

## Route Prefix
All public handlers map to routes with **no `/api` prefix**:
- Routes: `/auth/*`, `/docs/*`
- No JWT token required
- Available to anonymous users

## Handlers

### Authentication (`/auth/*`)
Token acquisition and account registration endpoints:

- **POST /auth/login** → `auth/login.rs`
  - Authenticate user and receive JWT token
  - Input: `{ "tenant": "string", "username": "string" }`
  - Output: JWT token + user info

- **POST /auth/register** → `auth/register.rs`  
  - Register new user account (if enabled)
  - Input: `{ "tenant": "string", "username": "string" }`
  - Output: Success confirmation

- **POST /auth/refresh** → `auth/refresh.rs`
  - Refresh expired JWT token
  - Input: `{ "token": "string" }`  
  - Output: New JWT token

## TypeScript Equivalent
```typescript
// monk-api/src/public/auth/routes.ts
export { default as LoginPost } from './login/POST.js';
export { default as RegisterPost } from './register/POST.js';  
export { default as RefreshPost } from './refresh/POST.js';
```

## Security Notes

- **No middleware applied** - completely public access
- **Input validation required** - no authenticated user context
- **Rate limiting recommended** - prevent brute force attacks on login
- **HTTPS required** - protect credentials in transit

## Usage Flow

1. **Anonymous user** calls `POST /auth/login` with credentials
2. **Server validates** tenant + username combination  
3. **JWT token issued** with user info and permissions
4. **Client stores token** for subsequent API calls to `/api/*` routes
5. **Token used** in Authorization header: `Bearer <token>`

This tier is the entry point for all user interactions with the API.