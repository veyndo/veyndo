"""Disposable real-transaction counterexamples for subscriber schedules and commit-order gaps."""
import os
import subprocess
from .database import psql


def command():
    return psql(os.environ['DB_CONTAINER'], 'commerce', os.environ['TEST_DATABASE'], '-XqAt', '-v', 'ON_ERROR_STOP=1')


def tenant_schedules():
    # Rollback-only checks on the set-up schema. Never run this fixture in a named
    # UI/restored/production database.
    assert os.environ['TEST_DATABASE'].startswith('commerce_quality_')
    body = """
BEGIN;
SELECT set_config('rac.system','on',true);
DO $$ BEGIN
 IF (SELECT count(*) FROM outbox_checkpoints)<>(SELECT count(*)*4 FROM tenants) THEN RAISE EXCEPTION 'Missing existing tenant schedules'; END IF;
 IF NOT EXISTS(SELECT 1 FROM pg_constraint WHERE conrelid='outbox_subscriber_failures'::regclass AND confrelid='outbox'::regclass AND array_length(conkey,1)=2) THEN RAISE EXCEPTION 'Missing tenant-bound pending reference'; END IF;
END $$;
DO $$ DECLARE foreign_event bigint; BEGIN
 INSERT INTO tenants(id,name) VALUES('fixture-outbox-foreign','Synthetic outbox boundary');
 IF (SELECT count(*) FROM outbox_checkpoints WHERE tenant='fixture-outbox-foreign')<>4 THEN RAISE EXCEPTION 'Missing new tenant schedules'; END IF;
 INSERT INTO outbox(tenant,kind,data) VALUES('fixture-outbox-foreign','fixture.foreign','{}') RETURNING id INTO foreign_event;
 BEGIN
  INSERT INTO outbox_subscriber_failures(tenant,event_id,subscriber) VALUES('workshop',foreign_event,'audit');
  RAISE EXCEPTION 'Foreign pending event was admitted';
 EXCEPTION WHEN foreign_key_violation THEN NULL;
 END;
END $$;
ROLLBACK;
"""
    subprocess.run(command(), input=body, text=True, check=True, stdout=subprocess.DEVNULL)
    print('PASS every tenant has its subscriber schedules and pending failures stay tenant-bound; checks rolled back', flush=True)


def late_commit(sql, wait):
    holder = subprocess.Popen(command(), stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        holder.stdin.write("BEGIN; SELECT set_config('rac.system','on',true); INSERT INTO outbox(tenant,kind,data) VALUES('workshop','fixture.late-commit','{}') RETURNING id;\n\\echo INSERT_HELD\n")
        holder.stdin.flush()
        earlier = None
        while True:
            line = holder.stdout.readline().strip()
            if line == 'INSERT_HELD':
                break
            if line.isdigit():
                earlier = int(line)
            if holder.poll() is not None:
                raise AssertionError('Held fixture transaction failed')
        assert earlier is not None
        later = int(sql("INSERT INTO outbox(tenant,kind,data) VALUES('workshop','fixture.early-commit','{}') RETURNING id"))
        assert later > earlier
        wait(lambda: sql(f"SELECT subscriber_mask=15 AND delivered_at IS NOT NULL FROM outbox WHERE id={later}") == 't')
        holder.stdin.write('COMMIT;\n'); holder.stdin.flush()
        holder.stdin.close(); holder.wait(timeout=10)
        assert holder.returncode == 0
        wait(lambda: sql(f"SELECT subscriber_mask=15 AND delivered_at IS NOT NULL FROM outbox WHERE id={earlier}") == 't')
        assert sql(f"SELECT count(*) FROM projections WHERE event_id IN ({earlier},{later})") == '2'
        print('PASS earlier sequence ID committed after all lanes passed a later ID is projected and exactly acknowledged', flush=True)
    finally:
        if holder.poll() is None:
            holder.terminate(); holder.wait(timeout=10)


def retained_quarantine(sql, wait):
    # Seed an audit lane already quarantined on this disposable poison event.
    # The real cognition consumer then fails its first attempt on the same event.
    # A fresh, retryable failure must not clear another subscriber's quarantine.
    event = int(sql("INSERT INTO outbox(tenant,kind,data,dead_letter_at) VALUES('workshop','order.placed','{}',now()) RETURNING id"))
    sql(f"UPDATE outbox SET dispatch_requires_retry=false WHERE id={event}; INSERT INTO outbox_subscriber_failures(tenant,event_id,subscriber,attempts,dead_letter_at) VALUES('workshop',{event},'audit',8,now())")
    wait(lambda: sql(f"SELECT attempts>0 FROM outbox_subscriber_failures WHERE tenant='workshop' AND subscriber='cognition' AND event_id={event}") == 't')
    assert sql(f"SELECT dead_letter_at IS NOT NULL FROM outbox WHERE id={event}") == 't', 'Another subscriber cleared the audit quarantine'
    # Repair only the synthetic input via the consumer's existing receipt fence.
    sql(f"INSERT INTO knowledge_receipts(tenant,event_id) VALUES('workshop',{event}) ON CONFLICT DO NOTHING")
    print('PASS retryable subscriber failure retains another subscriber quarantine', flush=True)
    return event


def locked_schedule(sql):
    """An event producer must not borrow the subscriber's scheduler lock."""
    holder = subprocess.Popen(command(), stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        holder.stdin.write("BEGIN; SELECT set_config('rac.system','on',true); UPDATE outbox_checkpoints SET served_order=served_order+1 WHERE tenant='workshop' AND subscriber='cognition';\n\\echo SCHEDULE_HELD\n")
        holder.stdin.flush()
        while True:
            if holder.stdout.readline().strip() == 'SCHEDULE_HELD':
                break
            if holder.poll() is not None:
                raise AssertionError('Subscriber schedule lock fixture failed')
        event = int(sql("SET LOCAL statement_timeout='2s'; INSERT INTO outbox(tenant,kind,data) VALUES('workshop','fixture.lock-independent','{}') RETURNING id"))
        assert event > 0
        print('PASS producer commits an event while the subscriber scheduling row is locked', flush=True)
    finally:
        if holder.poll() is None:
            holder.stdin.write('ROLLBACK;\n'); holder.stdin.flush(); holder.stdin.close(); holder.wait(timeout=5)
