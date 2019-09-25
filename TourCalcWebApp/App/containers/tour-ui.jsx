import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import { BrowserRouter as Router, Route, Switch, Link } from 'react-router-dom';

import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableRow from '@material-ui/core/TableRow';
import Paper from '@material-ui/core/Paper';

import PersonForm from './tour-person-edit.jsx'
import SpendingForm from './tour-spending-edit.jsx'
import SpendingsDetail from './person-show-spendings.jsx'


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
                return (<div><pre>{JSON.stringify(this.state.authData, null, 2)}</pre><TourRequestAccessCode app={this} /></div>)
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
            dialogPOpen: false
        }
    }

    componentDidMount() {
        AppState.loadTour(this, this.props.tourid);
    }

    render() {
        if (!this.state.isTourLoaded) {
            return <div>Tour {this.props.tourid} loading...</div>
        } else {
            return (
                <Router>
                    <div>
                        <h2>{this.state.tour.name}</h2>
                        <Link to={'/tour/' + this.props.tourid + '/persons'}>Persons</Link>&nbsp;
                        <Link to={'/tour/' + this.props.tourid + '/spendings'}>Spendings</Link>
                        <span><button onClick={() => { this.setState({ isTourLoaded: false }); AppState.loadTour(this, this.props.tourid); }}>Refresh</button> </span>

                        <main>
                            <Switch>
                                <Route path={'/tour/' + this.props.tourid + '/spendings'}
                                    render={(props) => (

                                        <Paper>
                                            <Table stickyHeader>
                                                <TableHead>
                                                    <TableRow>
                                                        <TableCell>Description (
                                                            <SpendingForm
                                                                tour={this.state.tour}
                                                                buttonText="Add"
                                                                actionButtonText="Add Spending"
                                                                open={false}
                                                                mode="add"
                                                                app={this}
                                                            />)
                                    
                                                        </TableCell>
                                                        <TableCell align="right">From</TableCell>
                                                        <TableCell align="right">Amount</TableCell>
                                                        <TableCell align="right">To all?</TableCell>
                                                        <TableCell align="right">Recipients</TableCell>
                                                    </TableRow>
                                                </TableHead>
                                                <TableBody>
                                                    {this.state.tour.spendings.map(p => (
                                                        <TableRow key={p.guid} hover>
                                                            <TableCell component="th" scope="row">
                                                                {p.description}
                                                                <SpendingForm
                                                                    tour={this.state.tour}
                                                                    buttonText="Edit"
                                                                    actionButtonText="Edit Spending"
                                                                    open={false}
                                                                    mode="edit"
                                                                    app={this}
                                                                    spending={p}
                                                                />
                                                                <button onClick={() => {
                                                                    if (window.confirm('Sure to delete spending ' + p.name + '?')) {
                                                                        AppState.deleteSpending(this, this.props.tourid, p.guid)
                                                                            .then(() => { AppState.loadTour(this, this.props.tourid); })
                                                                    }
                                                                }}>Del</button>
                                                            </TableCell>
                                                            <TableCell align="right">{
                                                                this.state.tour.persons.filter((pp) => pp.guid === p.fromGuid).map(ppp => ppp.name)
                                                            }</TableCell>
                                                            <TableCell align="right">
                                                                {p.amountInCents}
                                                            </TableCell>
                                                            <TableCell align="right">{p.toAll ? 'true' : 'false'}</TableCell>
                                                            <TableCell align="right">{p.toGuid.map(

                                                                (id) => this.state.tour.persons.find((pp) => pp.guid === id).name

                                                            ).join(', ')}</TableCell>
                                                        </TableRow>  
                                                    ))}
                                                </TableBody>
                                            </Table>
                                        </Paper>


                                    )} />
                                <Route path={'/tour/' + this.props.tourid}
                                    render={(props) => (
                                        <Paper>
                                            <Table stickyHeader>
                                                <TableHead>
                                                    <TableRow>
                                                        <TableCell>Name (
                                                            <PersonForm mode="add"
                                                                tourid={this.props.tourid}
                                                                open={false}
                                                                app={this}
                                                                buttonText="Add" actionButtonText="Add Person" />)
                                                            </TableCell>
                                                        <TableCell align="right">Weight %</TableCell>
                                                        <TableCell align="right">Spent</TableCell>
                                                        <TableCell align="right">Received</TableCell>
                                                        <TableCell align="right">Owes</TableCell>
                                                    </TableRow>
                                                </TableHead>
                                                <TableBody>
                                                    {this.state.tour.persons.map(p => (
                                                        <TableRow key={p.guid} hover>
                                                            <TableCell component="th" scope="row">
                                                                {p.name} 
                                                                <PersonForm mode="edit"
                                                                    tourid={this.props.tourid}
                                                                    open={false}
                                                                    app={this}
                                                                    buttonText="Edit" actionButtonText="Save Person"
                                                                    name={p.name}
                                                                    weight={p.weight}
                                                                    guid={p.guid}
                                                                />
                                                                <button onClick={() => {
                                                                    if (window.confirm('Sure to delete ' + p.name + '?')) {
                                                                        AppState.deletePerson(this, this.props.tourid, p.guid)
                                                                            .then(() => { AppState.loadTour(this, this.props.tourid); })
                                                                    }
                                                                }}>Del</button>
                                                            </TableCell>
                                                            <TableCell align="right">{p.weight}</TableCell>
                                                            <TableCell align="right">
                                                                <SpendingsDetail person={p} spendingInfo={p.spentSendingInfo} showText={p.spentInCents} open={false}
                                                                    received={false} />

                                                            </TableCell>
                                                            <TableCell align="right">
                                                                <SpendingsDetail person={p} spendingInfo={p.receivedSendingInfo} showText={p.receivedInCents} open={false}
                                                                    received={true}/>
                                                            </TableCell>
                                                            <TableCell align="right">{(p.receivedInCents - p.spentInCents)}</TableCell>
                                                        </TableRow>
                                                    ))}
                                                </TableBody>
                                            </Table>
                                        </Paper>
                                    )} />
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