// examples/openclaw_plugin.ts
// OpenClaw 插件：将技能执行转发给 agent-core

import fetch from 'node-fetch';

const AGENT_CORE_URL = process.env.AGENT_CORE_URL || 'http://localhost:3000';
const API_KEY = process.env.AGENT_CORE_API_KEY; // 可选，如果配置了API Key

export default {
  name: 'agent-core-executor',
  hooks: {
    async before_tool_call(toolCall, context) {
      // 构造请求体
      const body = {
        intent: {
          type: toolCall.name,
          payload: toolCall.arguments,
        },
        callback_url: context.session?.callbackUrl, // 如果有回调需求
      };

      const headers: any = {
        'Content-Type': 'application/json',
      };
      if (API_KEY) {
        headers['X-API-Key'] = API_KEY;
      }

      try {
        const response = await fetch(`${AGENT_CORE_URL}/run`, {
          method: 'POST',
          headers,
          body: JSON.stringify(body),
        });
        const result = await response.json();
        if (response.ok) {
          return result; // 返回结果给 OpenClaw
        } else {
          throw new Error(`agent-core error: ${result.status}`);
        }
      } catch (error) {
        console.error('Failed to call agent-core:', error);
        throw error;
      }
    },
  },
};