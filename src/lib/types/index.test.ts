import { describe, it, expect } from 'vitest';
import {
	getModelInfo,
	getModelsForProvider,
	getProviderForModel,
	getColumnForSession,
	MODEL_REGISTRY,
	COLUMN_ORDER,
	COLUMN_CONFIG,
	type Session,
	type ColumnId
} from './index';

describe('getModelInfo', () => {
	it('returns model info for a valid model id', () => {
		const info = getModelInfo('claude-opus-4-6');
		expect(info).toBeDefined();
		expect(info!.display_name).toBe('Opus 4.6');
		expect(info!.provider).toBe('claude');
	});

	it('returns undefined for an unknown model id', () => {
		expect(getModelInfo('nonexistent-model')).toBeUndefined();
	});
});

describe('getModelsForProvider', () => {
	it('returns only claude models for claude provider', () => {
		const models = getModelsForProvider('claude');
		expect(models.length).toBeGreaterThan(0);
		expect(models.every((m) => m.provider === 'claude')).toBe(true);
	});

	it('returns only codex models for codex provider', () => {
		const models = getModelsForProvider('codex');
		expect(models.length).toBeGreaterThan(0);
		expect(models.every((m) => m.provider === 'codex')).toBe(true);
	});

	it('returns only opencode models for opencode provider', () => {
		const models = getModelsForProvider('opencode');
		expect(models.length).toBeGreaterThan(0);
		expect(models.every((m) => m.provider === 'opencode')).toBe(true);
	});
});

describe('getProviderForModel', () => {
	it('returns the correct provider for a known model', () => {
		expect(getProviderForModel('gpt-5.4')).toBe('codex');
		expect(getProviderForModel('claude-sonnet-4-6')).toBe('claude');
		expect(getProviderForModel('openai/gpt-5.4')).toBe('opencode');
	});

	it('defaults to claude for an unknown model', () => {
		expect(getProviderForModel('unknown-model')).toBe('claude');
	});
});

describe('getColumnForSession', () => {
	function makeSession(stage: string): Session {
		return {
			id: 'test',
			repo_id: 1,
			issue_number: 1,
			stage: stage as Session['stage'],
			worktree_path: null,
			session_id: null,
			status: 'running',
			error_message: null,
			started_at: '2026-01-01T00:00:00Z',
			completed_at: null,
			hidden: false,
			cost_usd: null,
			provider: 'claude',
			model: 'claude-opus-4-6'
		};
	}

	it('maps spec stage to planning column', () => {
		expect(getColumnForSession(makeSession('spec'))).toBe('planning');
	});

	it('maps implement stage to in_progress column', () => {
		expect(getColumnForSession(makeSession('implement'))).toBe('in_progress');
	});

	it('maps ci_fix stage to in_progress column', () => {
		expect(getColumnForSession(makeSession('ci_fix'))).toBe('in_progress');
	});

	it('maps review stage to review column', () => {
		expect(getColumnForSession(makeSession('review'))).toBe('review');
	});

	it('maps merge_conflict stage to blocked column', () => {
		expect(getColumnForSession(makeSession('merge_conflict'))).toBe('blocked');
	});
});

describe('COLUMN_ORDER', () => {
	it('contains all column ids from COLUMN_CONFIG', () => {
		const configKeys = Object.keys(COLUMN_CONFIG) as ColumnId[];
		expect(COLUMN_ORDER).toHaveLength(configKeys.length);
		for (const key of configKeys) {
			expect(COLUMN_ORDER).toContain(key);
		}
	});
});

describe('MODEL_REGISTRY', () => {
	it('has unique model ids', () => {
		const ids = MODEL_REGISTRY.map((m) => m.id);
		expect(new Set(ids).size).toBe(ids.length);
	});

	it('every model has at least one effort level', () => {
		for (const model of MODEL_REGISTRY) {
			expect(model.effort_levels.length).toBeGreaterThan(0);
		}
	});
});
