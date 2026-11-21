-- Authored synthetic fixture. No original database, prompt, credential or invention.
PRAGMA foreign_keys=ON;
CREATE TABLE PFLFixture(key TEXT PRIMARY KEY,value TEXT NOT NULL);
INSERT INTO PFLFixture VALUES('synthetic','true'),('fixture_id','pfl-legacy-synthetic-v1');
CREATE TABLE Project(id TEXT PRIMARY KEY,title TEXT,status TEXT,createdAt INTEGER,updatedAt INTEGER);
CREATE TABLE InventionInput(
 id TEXT PRIMARY KEY,projectId TEXT UNIQUE REFERENCES Project(id),title TEXT,description TEXT,
 problemSolved TEXT,howItWorks TEXT,aiComponents TEXT,threeDPrintComponents TEXT,whatIsNovel TEXT,
 currentAlternatives TEXT,whatIsBuilt TEXT,whatToProtect TEXT,additionalNotes TEXT);
CREATE TABLE FeasibilityRun(
 id TEXT PRIMARY KEY,projectId TEXT REFERENCES Project(id),version INTEGER,status TEXT,
 startedAt INTEGER,completedAt INTEGER,finalReport TEXT);
CREATE TABLE FeasibilityStage(
 id TEXT PRIMARY KEY,feasibilityRunId TEXT REFERENCES FeasibilityRun(id),stageNumber INTEGER,
 stageName TEXT,status TEXT,outputText TEXT,model TEXT,webSearchUsed INTEGER,
 startedAt INTEGER,completedAt INTEGER,errorMessage TEXT,inputTokens INTEGER,outputTokens INTEGER,estimatedCostUsd REAL);
CREATE TABLE ClaimDraft(id TEXT PRIMARY KEY,projectId TEXT REFERENCES Project(id),version INTEGER,status TEXT,specLanguage TEXT,revisionNotes TEXT,estimatedCostUsd REAL);
CREATE TABLE Claim(id TEXT PRIMARY KEY,draftId TEXT REFERENCES ClaimDraft(id),claimNumber INTEGER,claimType TEXT,parentClaimNumber INTEGER,text TEXT,examinerNotes TEXT);
CREATE TABLE ComplianceCheck(id TEXT PRIMARY KEY,projectId TEXT REFERENCES Project(id),version INTEGER,draftVersion INTEGER,status TEXT,overallPass INTEGER,estimatedCostUsd REAL);
CREATE TABLE ComplianceResult(id TEXT PRIMARY KEY,checkId TEXT REFERENCES ComplianceCheck(id),rule TEXT,status TEXT,claimNumber INTEGER,detail TEXT,citation TEXT,suggestion TEXT);
CREATE TABLE PatentApplication(id TEXT PRIMARY KEY,projectId TEXT REFERENCES Project(id),version INTEGER,status TEXT,title TEXT,detailedDescription TEXT,crossReferences TEXT,idsTable TEXT,estimatedCostUsd REAL);
CREATE TABLE PriorArtSearch(id TEXT PRIMARY KEY,projectId TEXT REFERENCES Project(id),version INTEGER,status TEXT,feasibilityRunId TEXT);
CREATE TABLE PriorArtResult(id TEXT PRIMARY KEY,searchId TEXT REFERENCES PriorArtSearch(id),patentNumber TEXT,title TEXT,relevanceScore REAL,claimMapping TEXT,source TEXT);
CREATE TABLE PatentDetail(id TEXT PRIMARY KEY,patentNumber TEXT UNIQUE,assignee TEXT,inventors TEXT,cpcClassifications TEXT,claimsText TEXT);
CREATE TABLE PatentFamily(id TEXT PRIMARY KEY,patentNumber TEXT UNIQUE,members TEXT);
CREATE TABLE ProsecutionEvent(id TEXT PRIMARY KEY,projectId TEXT REFERENCES Project(id),eventType TEXT,eventDate INTEGER,description TEXT,documentUrl TEXT);
CREATE TABLE OdpApiUsage(id TEXT PRIMARY KEY,projectId TEXT REFERENCES Project(id) ON DELETE SET NULL,queriesAttempted INTEGER,resultsFound INTEGER,hadRateLimit INTEGER,hadError INTEGER,errorMessage TEXT,calledAt INTEGER);
CREATE TABLE AppSettings(
 id TEXT PRIMARY KEY,provider TEXT,cloudApiKey TEXT,cloudDefaultModel TEXT,localDefaultModel TEXT,
 installEdition TEXT,ollamaUrl TEXT,ollamaModel TEXT,exportPath TEXT,autoExport INTEGER,
 usptoApiKey TEXT,encryptionSalt TEXT,researchModel TEXT,maxTokens INTEGER,interStageDelaySeconds INTEGER);
INSERT INTO Project VALUES('synthetic-project','Synthetic foldable marker','FEASIBILITY_COMPLETE',1000,9000);
INSERT INTO InventionInput VALUES(
 'synthetic-invention','synthetic-project','Synthetic foldable marker',
 'Entirely fictional café marker for migration tests: 日本語, α, and 🧪.',
 'Synthetic problem','Synthetic mechanism','Synthetic AI components','Synthetic physical components','Synthetic novelty',
 'Synthetic alternatives','Synthetic built state','Synthetic scope','Edited notes preserved verbatim');
INSERT INTO FeasibilityRun VALUES
 ('synthetic-run-v1','synthetic-project',1,'COMPLETE',2000,8000,'# Synthetic saved report v1

Edited café report. 日本語 🧪. This is not a real invention.'),
 ('synthetic-run-v2','synthetic-project',2,'RUNNING',8500,NULL,NULL);
INSERT INTO FeasibilityStage VALUES
 ('s1','synthetic-run-v1',1,'Technical restatement','COMPLETE','Synthetic stage 1','fixture-model',0,2000,2500,NULL,10,20,0.001),
 ('s2','synthetic-run-v1',2,'Prior art','COMPLETE','Synthetic stage 2','fixture-model',0,2500,3000,NULL,NULL,NULL,NULL),
 ('s3','synthetic-run-v1',3,'Patentability','COMPLETE','Synthetic stage 3','fixture-model',0,3000,3500,NULL,30,40,0.002),
 ('s4','synthetic-run-v1',4,'Landscape','COMPLETE','Synthetic stage 4','fixture-model',0,3500,4000,NULL,40,50,0.003),
 ('s5','synthetic-run-v1',5,'IP strategy','COMPLETE','Synthetic stage 5','fixture-model',0,4000,4500,NULL,50,60,0.004),
 ('s6','synthetic-run-v1',6,'Final report','COMPLETE','Synthetic stage 6','fixture-model',0,4500,8000,NULL,60,70,0.005),
 ('s7','synthetic-run-v2',1,'Technical restatement','COMPLETE','Synthetic version 2 stage 1','fixture-model',0,8500,8700,NULL,11,21,0.0011),
 ('s8','synthetic-run-v2',2,'Prior art','RUNNING','Partial uncommitted-to-checkpoint text','fixture-model',0,8700,NULL,'Synthetic interruption',NULL,NULL,NULL);
INSERT INTO ClaimDraft VALUES
 ('draft-v1','synthetic-project',1,'COMPLETED','Synthetic specification v1',NULL,0.01),
 ('draft-v2','synthetic-project',2,'COMPLETED','Edited synthetic specification v2','User edit retained',0.02);
INSERT INTO Claim VALUES
 ('claim-1','draft-v2',1,'INDEPENDENT',NULL,'Synthetic claim 1','Synthetic note'),
 ('claim-2','draft-v2',2,'DEPENDENT',1,'Synthetic claim 2 depends on claim 1','Edited examiner note');
INSERT INTO ComplianceCheck VALUES('check-v1','synthetic-project',1,2,'COMPLETED',0,0.003);
INSERT INTO ComplianceResult VALUES('finding-1','check-v1','definiteness','WARN',2,'Synthetic detail','Synthetic citation','Synthetic suggestion');
INSERT INTO PatentApplication VALUES
 ('application-v1','synthetic-project',1,'COMPLETED','Synthetic title','Original synthetic section','Claim 1','[{"synthetic":true}]',0.03),
 ('application-v2','synthetic-project',2,'COMPLETED','Edited synthetic title','Edited section: café 日本語 🧪','Claim 2','[{"synthetic":true}]',0.04);
INSERT INTO PriorArtSearch VALUES('search-1','synthetic-project',1,'COMPLETED','synthetic-run-v1');
INSERT INTO PriorArtResult VALUES('result-1','search-1','SYNTHETIC-0001','Synthetic reference',0.875,'{"2":"synthetic dependency"}','SYNTHETIC');
INSERT INTO PatentDetail VALUES('detail-1','SYNTHETIC-0001','["Synthetic Assignee"]','["Synthetic Inventor"]','["SYNTHETIC"]','Synthetic claim text');
INSERT INTO PatentFamily VALUES('family-1','SYNTHETIC-0001','[{"patentNumber":"SYNTHETIC-0002"}]');
INSERT INTO ProsecutionEvent VALUES('event-1','synthetic-project','SYNTHETIC',7000,'Synthetic prosecution record',NULL);
INSERT INTO OdpApiUsage VALUES
 ('usage-1','synthetic-project',2,1,0,0,NULL,7100),
 ('usage-global',NULL,1,0,1,1,'Synthetic rate limit',7200);
-- Deliberately fake opaque ciphertext, not an API key. Import never decrypts it.
INSERT INTO AppSettings VALUES(
 'singleton','LOCAL','0000000000000000000000000000000000000000000000000000000000',
 'synthetic-cloud-model','synthetic-existing-model','Full','http://127.0.0.1:11434',
 'synthetic-existing-model','synthetic-export-path',1,
 '1111111111111111111111111111111111111111111111111111111111',
 'synthetic-not-a-real-encryption-salt','synthetic-research-model',32000,5);
