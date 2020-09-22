import React from 'react';
import AppState from './appstate.jsx'

import { BrowserRouter as Router, Route, Switch, Link, Redirect } from 'react-router-dom';

import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableRow from '@material-ui/core/TableRow';
import Paper from '@material-ui/core/Paper';

import PersonForm from './tour-person-edit.jsx'
import SpendingForm from './tour-spending-edit.jsx'
import SpendingsDetail from './person-show-spendings.jsx'

import Checkbox from '@material-ui/core/Checkbox';

import createBrowserHistory from 'history/createBrowserHistory';

import Tabs from '@material-ui/core/Tabs';
import Tab from '@material-ui/core/Tab';
import Button from '@material-ui/core/Button';
import Box from '@material-ui/core/Box';

import { sizing } from '@material-ui/system';

import TourInfo from './tour-info.jsx'

const history = createBrowserHistory();


export default class TourUI extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isAuthLoaded: false,
        }
    }

    componentDidMount() {
        AppState.checkWhoAmI(this);
    }

    render() {
        if (!this.state.isAuthLoaded) {
            return (<div>Checking Who you are...</div>)
        } else {
            if (!this.state.authData.isMaster
                && this.state.authData.tourIds.indexOf(this.props.tourid) == -1) { // no such tour for credentials
                //return (<TourRequestAccessCode app={this} />)
                //alert("authdata: " + JSON.stringify(this.state.authData));
                if (this.state.authData.type == "AccessCode") { // just wrong access code, or tour is in another code
                    return <Redirect to="/" />
                } else {
                    return (<TourRequestAccessCode app={this} />)
                }

            } else {
                return (<TourTable tourid={this.props.tourid} />)
            }

        }

    }
}
/*const useStyles = makeStyles(theme => ({
    root: {
        width: '100%',
        marginTop: theme.spacing(3),
        overflowX: 'auto',
    },
    table: {
        minWidth: 650,
    },
}));
const classes = useStyles();*/

class TourTable extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isTourLoaded: false,
            tour: null,
            showSuggested: false,
            dialogPOpen: false,
            currentPage: 'persons',
            updateTime: new Date()
        }
    }

    componentDidMount() {
        document.title = "Toucalc: Tour loading"
        AppState.loadTour(this, this.props.tourid);
        this.interval = setInterval(
            () => {
                AppState.loadTour(this, this.props.tourid)
                    .then(this.setState({ updateTime: new Date() }))
            }
            , (60000 * 3)) // once a 3 min
    }

    componentWillUnmount() {
        clearInterval(this.interval);
    }

    render() {
        if (!this.state.isTourLoaded) {
            return <div>Tour {this.props.tourid} loading...</div>
        } else {

            if (this.state.tour == null) { // loaded, but null (no such tour)
                return <Redirect to="/"/>
            }

            document.title = "Tourcalc: " + this.state.tour.name;

            return (
                <Router>
                    <div style={{ maxWidth: 1080, border: "0px double black"}}>

                        {/*--- Tabs ---*/}
                        <Tabs
                            value={(history.location.pathname === '/' || !history.location.pathname.startsWith('/tour/' + this.props.tourid)) ? '/tour/' + this.props.tourid + '/persons' : history.location.pathname}
                            indicatorColor="primary"
                            textColor="primary"
                            onChange={(event, v) => { history.push(v); this.setState({ currentPage: v }) }}
                            aria-label="Tourcalc nav">

                            <Tab label="Persons" value={'/tour/' + this.props.tourid + '/persons'}
                                component={Link} to={'/tour/' + this.props.tourid + '/persons'}
                            />
                            <Tab label="Spendings" value={'/tour/' + this.props.tourid + '/spendings'}
                                component={Link} to={'/tour/' + this.props.tourid + '/spendings'}
                            />
                        </Tabs>
                        {/*--- /Tabs ---*/}


                        &nbsp;<TourInfo tour={this.state.tour} app={this} updateTime={this.state.updateTime} />

                        <main>
                            <Switch>
                                <Route exact path={'/tour/' + this.props.tourid}
                                    render={(props) => <Redirect to={'/tour/' + this.props.tourid + '/persons'} />}
                                />
                                {/*--- Spendings ---*/}

                                <Route path={'/tour/' + this.props.tourid + '/spendings'}
                                    render={(props) => (

                                        <Table stickyHeader>
                                            <TableHead>
                                                <TableRow>
                                                    <TableCell>Spending Description
                                                   { this.state.tour.isVersion ? <span/> :
                                                            <SpendingForm
                                                                tour={this.state.tour}
                                                                buttonText="Add"
                                                                actionButtonText="Add Spending"
                                                                open={false}
                                                                mode="add"
                                                                app={this}
                                                            ><Button color='primary' variant='outlined'>Add</Button></SpendingForm>
                                                        }
                                                        &nbsp;
                                                        <Button color='secondary' variant='outlined'
                                                            onClick={() => { this.setState({ showSuggested: !this.state.showSuggested }) }}>
                                                            S: {this.state.showSuggested ? 'hide' : 'show'}
                                                        </Button>

                                                    </TableCell>
                                                    <TableCell align="right">From</TableCell>
                                                    <TableCell align="right">Amount</TableCell>
                                                    <TableCell align="right">To all</TableCell>
                                                    <TableCell align="right">Recipients</TableCell>
                                                </TableRow>
                                            </TableHead>
                                            <TableBody>
                                                {this.state.tour.spendings

                                                    .sort((s1, s2) => {
                                                        if (s1.planned && !s2.planned) return -1;
                                                        if (!s1.planned && s2.planned) return 1;
                                                        if (s1.planned && s2.planned) {
                                                            if (s1.amountInCents > s2.amountInCents) return -1;
                                                            if (s1.amountInCents < s2.amountInCents) return 1;
                                                            return 0;
                                                        }
                                                        if (s1.dateCreated > s2.dateCreated) return 1;
                                                        if (s1.dateCreated < s2.dateCreated) return -1;
                                                        return 0;
                                                    })

                                                    .filter((sp) => this.state.showSuggested || !sp.planned)
                                                    .map((p, idx) => (
                                                        <TableRow key={p.guid} hover

                                                            style={p.planned ? (p.description.startsWith('Family') ? { background: "cyan" } : { background: "yellow" }) : { }}

                                                            selected={idx % 2 == 0 ? true : false}
                                                        >
                                                            <TableCell component="th" scope="row">

                                                                <span style={{ cursor: 'pointer', borderStyle: p.planned ? 'none' : 'ridge', fontSize: 'xx-small' }}
                                                                    variant='outlined' color='secondary' onClick={() => {
                                                                        if (window.confirm('Sure to delete spending ' + p.description + '?')) {
                                                                            AppState.deleteSpending(this, this.props.tourid, p.guid)
                                                                                .then(() => { AppState.loadTour(this, this.props.tourid); })
                                                                        }
                                                                    }}>{p.planned ? '' : 'X'}</span>

                                                                &nbsp;
                                                                {(idx + 1) + '.'}
                                                                {(!p.planned) ?
                                                                    <SpendingForm
                                                                        tour={this.state.tour}
                                                                        buttonText={p.description}
                                                                        actionButtonText="Save Spending"
                                                                        open={false}
                                                                        mode="edit"
                                                                        app={this}
                                                                        spending={p}
                                                                    ><span style={{ cursor: 'pointer', textDecoration: 'underline' }}>{p.description}</span></SpendingForm>
                                                                    : <span>{p.description}&nbsp;

                                                                        {this.state.tour.isVersion ? <span /> :
                                                                            <SpendingForm
                                                                                tour={this.state.tour}
                                                                                buttonText="Add"
                                                                                actionButtonText="Save Spending"
                                                                                open={false}
                                                                                mode="add"
                                                                                app={this}
                                                                                spending={p}
                                                                            ><Button color='primary' variant='outlined'>Add</Button></SpendingForm>
                                                                        }

                                                                    </span>
                                                                }
                                                            </TableCell>
                                                            <TableCell align="right">{
                                                                this.state.tour.persons.filter((pp) => pp.guid === p.fromGuid).map(ppp => ppp.name)
                                                            }</TableCell>
                                                            <TableCell align="right">
                                                                {p.amountInCents}
                                                            </TableCell>
                                                            <TableCell align="right"><Checkbox checked={p.toAll} disabled /></TableCell>
                                                            <TableCell align="right" style={{ fontSize: "xx-small" }}>{p.toAll ? (<i style={{ backgroundColor: 'green', color: 'yellow' }}>ALL</i>) : p.toGuid.map(

                                                                (id) => (this.state.tour.persons.find((pp) => pp.guid === id) == null)
                                                                    ? '$$-' + id + '-$$'
                                                                    : this.state.tour.persons.find((pp) => pp.guid === id).name

                                                            ).join(', ')}</TableCell>
                                                        </TableRow>
                                                    ))}
                                            </TableBody>
                                        </Table>


                                    )} />

                                {/*--- /Spendings ---*/}


                                {/*--- Persons ---*/}

                                <Route path={'/tour/' + this.props.tourid + '/persons'}
                                    render={(props) => (
                                        <Table stickyHeader>
                                            <TableHead>
                                                <TableRow>
                                                    <TableCell>Person Name
                                                        {this.state.tour.isVersion ? <span /> :
                                                            <PersonForm mode="add"
                                                                tourid={this.props.tourid}
                                                                tour={this.state.tour}
                                                                open={false}
                                                                app={this}
                                                                buttonText="Add" actionButtonText="Add Person" ><Button color='primary' variant='outlined'>Add</Button>
                                                            </PersonForm>
                                                        }
                                                    </TableCell>
                                                    <TableCell align="right">#</TableCell>
                                                    <TableCell align="right">Weight %</TableCell>
                                                    <TableCell align="right">Spent</TableCell>
                                                    <TableCell align="right">Received</TableCell>
                                                    <TableCell align="right">Debt</TableCell>
                                                </TableRow>
                                            </TableHead>
                                            <TableBody>
                                                {this.state.tour.persons.map((p, idx) => (
                                                    <TableRow key={p.guid} hover={true} selected={idx%2==0 ? true : false}>
                                                        <TableCell component="th" scope="row">
                                                            <span style={{ cursor: 'pointer', borderStyle: 'ridge', fontSize: 'xx-small' }} onClick={() => {
                                                                if (window.confirm('Sure to delete ' + p.name + '?')) {
                                                                    AppState.deletePerson(this, this.props.tourid, p.guid)
                                                                        .then(() => { AppState.loadTour(this, this.props.tourid); })
                                                                }
                                                            }}>X</span>
                                                            &nbsp;
                                                            {(idx + 1) + '.'}
                                                            <PersonForm mode="edit"
                                                                tourid={this.props.tourid}
                                                                tour={this.state.tour}
                                                                open={false}
                                                                app={this}
                                                                buttonText={p.name} actionButtonText="Save Person"
                                                                person={p}
                                                                guid={p.guid}
                                                            ><span style={{ cursor: 'pointer', textDecoration: 'underline' }}>{p.name}

                                                                </span></PersonForm>
                                                            <span style={{ fontSize: 'xx-small' }}>
                                                                {this.state.tour.persons.find((pp) => pp.guid == p.parentId) == null
                                                                    ? ''
                                                                    : ' > ' + this.state.tour.persons.find((pp) => pp.guid == p.parentId).name}
                                                            </span>
                                                        </TableCell>
                                                        <TableCell align="right">
                                                            {this.state.tour.isVersion ? <span /> :
                                                                <SpendingForm
                                                                    tour={this.state.tour}
                                                                    buttonText="Add"
                                                                    actionButtonText="Save Spending"
                                                                    open={false}
                                                                    mode="add"
                                                                    app={this}
                                                                    spending={{
                                                                        description: "",
                                                                        amountInCents: 0,
                                                                        fromGuid: p.guid,
                                                                        toGuid: [],
                                                                        toAll: false,
                                                                        guid: ""
                                                                    }}
                                                                ><Button color='primary' variant='outlined'
                                                                >Spend</Button></SpendingForm>
                                                            }
                                                        </TableCell>
                                                        <TableCell align="right">{p.weight}</TableCell>
                                                        <TableCell align="right">
                                                            <SpendingsDetail person={p} spendingInfo={p.spentSendingInfo} showText={p.spentInCents} open={false}
                                                                received={false} />

                                                        </TableCell>
                                                        <TableCell align="right">
                                                            <SpendingsDetail person={p} spendingInfo={p.receivedSendingInfo} showText={p.receivedInCents} open={false}
                                                                received={true} />
                                                        </TableCell>
                                                        <TableCell align="center" style={
                                                            (p.receivedInCents - p.spentInCents) == 0
                                                            ? {}
                                                            : { backgroundColor: (p.receivedInCents - p.spentInCents) <= 0 ? '#EAFAF1' : '#F9EBEA' }

                                                        }>
                                                            {(p.receivedInCents - p.spentInCents) != 0 ?
                                                                (p.receivedInCents - p.spentInCents) < 0
                                                                    ? <span style={{ color: "green", fontWeight: "bold" }}>{(p.receivedInCents - p.spentInCents)}</span>
                                                                    : <span style={{ color: "red", fontWeight: "bold" }}>{(p.receivedInCents - p.spentInCents)}</span>
                                                                : <span style={{ color: "black", fontWeight: "bold" }}>{(p.receivedInCents - p.spentInCents)}</span>
                                                            }
                                                        </TableCell>
                                                    </TableRow>
                                                ))}
                                            </TableBody>
                                        </Table>
                                    )} />
                                {/*--- /Persons ---*/}

                            </Switch>
                        </main>
                    </div>
                </Router>

            )
            //return <div><pre>Tour: {JSON.stringify(this.state.tour, null, 2)}}</pre></div>
        }
        function TPersons() {
            return <div> persons: <pre>{JSON.stringify(this.state.tour.persons, null, 2)}}</pre></div>
        }
        function TSpendings() {
            return <div> spendings: <pre>{JSON.stringify(this.state.tour.spendings, null, 2)}}</pre></div>
        }
    }

}

class TourRequestAccessCode extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isAuthLoaded: false
        }
    }

    code = 'wrong_code'

    render() {
        return (
            <div>
                <form onSubmit={(event) => {
                    event.preventDefault();
                    AppState.login(this.props.app, 'code', this.code)
                }}>
                    <p>access code:</p>
                    <input
                        type='text'
                        onChange={(e) => this.code = event.target.value}
                    />
                    <input
                        type='submit'
                    />
                </form>
            </div>
        )
    }

}